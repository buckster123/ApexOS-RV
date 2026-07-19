//! The goal driver — a fresh `no_std` implementation of the semantics in
//! upstream `agentd/crates/agentd/src/goal.rs` @ 676aa38 (design ancestor:
//! docs/upstream/goal-driver-design.md — "LLM-proposes / code-disposes").
//!
//! Fidelity map (CLAUDE.md rule 7 — behaviors come from source, not intuition):
//!
//! - `Verdict` mirrors upstream verbatim: `Continue(Option<steer>)` / `Done` /
//!   `Blocked(reason)`. A turn that completes without reporting a verdict is
//!   `Continue(None)` (upstream: `pending.take().unwrap_or(Continue(None))`).
//! - `step` is the **in-flight** step, 1-indexed; a new goal starts `Acting`
//!   at step 1 with detail `""` (upstream `create_goal`), and `max_steps`
//!   clamps to `1..=MAX_STEPS_CEIL` exactly like `goal_create`.
//! - Applying `Continue` at `step >= max_steps` finishes the goal as **Done**
//!   with detail `"step budget reached"` — budget exhaustion is completion,
//!   not failure. Otherwise `step += 1` and `Acting` re-emits with detail `""`.
//! - `Done` finishes with detail `""`; `Blocked(reason)` parks with the reason
//!   as detail. Upstream resume (`goal_resume`, detail `"resumed"`) is an
//!   operator action — v1 metal never exercises it.
//! - A stalled turn (upstream watchdog: no `TurnComplete` within the step
//!   timeout) fails the goal with detail `"step stalled — no completion"`.
//!   Stall is an *observation of the turn engine*, not an LLM verdict — hence
//!   the separate `TurnResult` channel.
//! - `Planning`/`Reflecting` are never emitted (reserved-unused upstream);
//!   `Cancelled` is operator-only; the approval-block path (`"awaiting
//!   approval — {tool}"`) needs a policy gate that v1 metal doesn't have.
//! - Every transition emits `Event::GoalStateChanged` with upstream's exact
//!   field set; `yolo` is always `false` in v1.

use alloc::string::String;
use apexos_protocol::{Event, GoalId, GoalState};

/// Upstream budget defaults (`goal.rs` @ the pin).
pub const DEFAULT_MAX_STEPS: u32 = 12;
pub const MAX_STEPS_CEIL: u32 = 100;

/// The agent's reported outcome for the in-flight step (upstream `goal_step`).
pub enum Verdict {
    Continue(Option<String>), // optional steer for the next step
    Done,
    Blocked(String), // reason
}

/// How the turn engine observed the in-flight step end.
pub enum TurnResult {
    Completed(Verdict),
    Stalled,
}

/// What the driver shows the inference for one step.
pub struct TickContext<'a> {
    pub goal: GoalId,
    pub objective: &'a str,
    pub step: u32,
    pub max_steps: u32,
    /// Steer from the previous step's `Continue(Some(_))` — surfaced the way
    /// upstream folds it into the next step's directive prompt.
    pub steer: Option<&'a str>,
}

/// The inference hook. v1 ships [`ScriptedInference`]; post-v1 this is where
/// the mesh client goes.
pub trait Inference {
    fn next(&mut self, ctx: &TickContext<'_>) -> TurnResult;
}

pub struct GoalDriver {
    goal: GoalId,
    objective: String,
    state: GoalState,
    step: u32,
    max_steps: u32,
    steer: Option<String>,
}

impl GoalDriver {
    /// Create the goal and emit the initial `Acting(1, "")` (upstream
    /// `create_goal`). The emit sink is the kernel's serializer in production
    /// and a `Vec` collector in tests — the driver itself never does I/O.
    pub fn start(
        goal: GoalId,
        objective: &str,
        max_steps: u32,
        emit: &mut dyn FnMut(&Event),
    ) -> Self {
        let d = Self {
            goal,
            objective: String::from(objective),
            state: GoalState::Acting,
            step: 1,
            max_steps: max_steps.clamp(1, MAX_STEPS_CEIL),
            steer: None,
        };
        d.emit_state(emit, "");
        d
    }

    pub fn state(&self) -> GoalState {
        self.state
    }

    fn emit_state(&self, emit: &mut dyn FnMut(&Event), detail: &str) {
        emit(&Event::GoalStateChanged {
            goal: self.goal,
            objective: self.objective.clone(),
            state: self.state,
            step: self.step,
            max_steps: self.max_steps,
            detail: String::from(detail),
            yolo: false,
        });
    }

    /// Apply one observed turn end (upstream `advance` + `fail_stalled`).
    /// Only meaningful while `Acting`; a non-Acting driver ignores the call
    /// (upstream's lookup filters on `state == Acting`).
    pub fn advance(&mut self, result: TurnResult, emit: &mut dyn FnMut(&Event)) {
        if !matches!(self.state, GoalState::Acting) {
            return;
        }
        self.steer = None;
        match result {
            TurnResult::Stalled => {
                self.state = GoalState::Failed;
                self.emit_state(emit, "step stalled — no completion");
            }
            TurnResult::Completed(Verdict::Done) => {
                self.state = GoalState::Done;
                self.emit_state(emit, "");
            }
            TurnResult::Completed(Verdict::Blocked(reason)) => {
                self.state = GoalState::Blocked;
                self.emit_state(emit, &reason);
            }
            TurnResult::Completed(Verdict::Continue(steer)) => {
                if self.step >= self.max_steps {
                    self.state = GoalState::Done; // budget reached
                    self.emit_state(emit, "step budget reached");
                } else {
                    self.step += 1;
                    self.steer = steer;
                    self.emit_state(emit, "");
                }
            }
        }
    }

    /// Drive until the goal leaves `Acting`; returns the parked/terminal state.
    pub fn run<I: Inference>(mut self, inf: &mut I, emit: &mut dyn FnMut(&Event)) -> GoalState {
        while matches!(self.state, GoalState::Acting) {
            let steer = self.steer.take();
            let result = {
                let ctx = TickContext {
                    goal: self.goal,
                    objective: &self.objective,
                    step: self.step,
                    max_steps: self.max_steps,
                    steer: steer.as_deref(),
                };
                inf.next(&ctx)
            };
            self.advance(result, emit);
        }
        self.state
    }
}

// ── Scripted inference (P6.3) ────────────────────────────────────────────────

/// One transcript entry — `&'static` payloads so transcripts are plain consts.
pub enum ScriptStep {
    Continue,
    ContinueWith(&'static str),
    Done,
    Blocked(&'static str),
    Stall,
}

/// Deterministic, compiled-in "LLM": replays a transcript. An exhausted script
/// reads as the turn engine going quiet — stall semantics, never a hang.
pub struct ScriptedInference {
    steps: &'static [ScriptStep],
    at: usize,
}

impl ScriptedInference {
    pub const fn new(steps: &'static [ScriptStep]) -> Self {
        Self { steps, at: 0 }
    }
}

impl Inference for ScriptedInference {
    fn next(&mut self, _ctx: &TickContext<'_>) -> TurnResult {
        let step = self.steps.get(self.at);
        self.at += 1;
        match step {
            Some(ScriptStep::Continue) => TurnResult::Completed(Verdict::Continue(None)),
            Some(ScriptStep::ContinueWith(s)) => {
                TurnResult::Completed(Verdict::Continue(Some(String::from(*s))))
            }
            Some(ScriptStep::Done) => TurnResult::Completed(Verdict::Done),
            Some(ScriptStep::Blocked(r)) => {
                TurnResult::Completed(Verdict::Blocked(String::from(*r)))
            }
            Some(ScriptStep::Stall) | None => TurnResult::Stalled,
        }
    }
}

/// v1 transcripts (PLAN P6.3). Success: a three-step walk finishing early via
/// an explicit `Done` (upstream P2b path). Stall: step 2 never completes.
pub const SCRIPT_SUCCESS: &[ScriptStep] = &[
    ScriptStep::Continue,
    ScriptStep::ContinueWith("summarize sensor coverage, then wrap up"),
    ScriptStep::Done,
];
pub const SCRIPT_STALL: &[ScriptStep] = &[ScriptStep::Continue, ScriptStep::Stall];

// ── Host tests (P6.6): one per FR-8 behavior ─────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn collect(
        max_steps: u32,
        script: &'static [ScriptStep],
    ) -> (GoalState, Vec<(GoalState, u32, String, bool)>) {
        let mut events = Vec::new();
        let mut sink = |ev: &Event| match ev {
            Event::GoalStateChanged { state, step, detail, yolo, .. } => {
                events.push((*state, *step, detail.clone(), *yolo));
            }
            other => panic!("driver emitted a non-goal event: {other:?}"),
        };
        let mut inf = ScriptedInference::new(script);
        let driver = GoalDriver::start(GoalId(1), "test objective", max_steps, &mut sink);
        let terminal = driver.run(&mut inf, &mut sink);
        (terminal, events)
    }

    #[test]
    fn success_walk_acting_1_2_3_then_done_with_empty_details() {
        let (terminal, ev) = collect(8, SCRIPT_SUCCESS);
        assert_eq!(terminal, GoalState::Done);
        let expect = [
            (GoalState::Acting, 1, ""),
            (GoalState::Acting, 2, ""),
            (GoalState::Acting, 3, ""),
            (GoalState::Done, 3, ""),
        ];
        assert_eq!(ev.len(), expect.len());
        for ((state, step, detail, _), (es, en, ed)) in ev.iter().zip(expect) {
            assert_eq!((*state, *step, detail.as_str()), (es, en, ed));
        }
    }

    #[test]
    fn budget_exhaustion_is_done_not_failed_with_upstream_detail() {
        const ALL_CONTINUE: &[ScriptStep] =
            &[ScriptStep::Continue, ScriptStep::Continue, ScriptStep::Continue];
        let (terminal, ev) = collect(2, ALL_CONTINUE);
        assert_eq!(terminal, GoalState::Done);
        let last = ev.last().unwrap();
        assert_eq!(
            (last.0, last.1, last.2.as_str()),
            (GoalState::Done, 2, "step budget reached")
        );
    }

    #[test]
    fn blocked_parks_with_reason_as_detail() {
        const BLOCKS: &[ScriptStep] =
            &[ScriptStep::Continue, ScriptStep::Blocked("waiting on operator hardware")];
        let (terminal, ev) = collect(8, BLOCKS);
        assert_eq!(terminal, GoalState::Blocked);
        let last = ev.last().unwrap();
        assert_eq!(
            (last.0, last.1, last.2.as_str()),
            (GoalState::Blocked, 2, "waiting on operator hardware")
        );
    }

    #[test]
    fn stall_fails_with_upstream_detail() {
        let (terminal, ev) = collect(8, SCRIPT_STALL);
        assert_eq!(terminal, GoalState::Failed);
        let last = ev.last().unwrap();
        assert_eq!(
            (last.0, last.1, last.2.as_str()),
            (GoalState::Failed, 2, "step stalled — no completion")
        );
    }

    #[test]
    fn exhausted_script_reads_as_stall_never_hangs() {
        const SHORT: &[ScriptStep] = &[ScriptStep::Continue];
        let (terminal, ev) = collect(8, SHORT);
        assert_eq!(terminal, GoalState::Failed);
        assert_eq!(ev.last().unwrap().2, "step stalled — no completion");
    }

    #[test]
    fn steer_surfaces_in_the_next_tick_context() {
        struct Probe {
            saw: Vec<Option<String>>,
        }
        impl Inference for Probe {
            fn next(&mut self, ctx: &TickContext<'_>) -> TurnResult {
                self.saw.push(ctx.steer.map(String::from));
                match self.saw.len() {
                    1 => TurnResult::Completed(Verdict::Continue(Some("focus".into()))),
                    2 => TurnResult::Completed(Verdict::Continue(None)),
                    _ => TurnResult::Completed(Verdict::Done),
                }
            }
        }
        let mut probe = Probe { saw: Vec::new() };
        let mut sink = |_: &Event| {};
        let driver = GoalDriver::start(GoalId(2), "steer test", 8, &mut sink);
        assert_eq!(driver.run(&mut probe, &mut sink), GoalState::Done);
        assert_eq!(probe.saw, vec![None, Some("focus".into()), None]);
    }

    #[test]
    fn max_steps_clamps_like_goal_create() {
        let mut sink = |_: &Event| {};
        let d = GoalDriver::start(GoalId(3), "clamp", 0, &mut sink);
        assert_eq!(d.max_steps, 1);
        let d = GoalDriver::start(GoalId(3), "clamp", 1000, &mut sink);
        assert_eq!(d.max_steps, MAX_STEPS_CEIL);
    }

    #[test]
    fn only_upstream_exercised_states_and_yolo_false_everywhere() {
        for script in [SCRIPT_SUCCESS, SCRIPT_STALL] {
            let (_, ev) = collect(8, script);
            for (state, _, _, yolo) in ev {
                assert!(
                    matches!(
                        state,
                        GoalState::Acting | GoalState::Blocked | GoalState::Done | GoalState::Failed
                    ),
                    "driver must never emit reserved/operator states, got {state:?}"
                );
                assert!(!yolo);
            }
        }
    }

    #[test]
    fn emitted_events_serialize_with_the_wire_contract() {
        let mut lines = Vec::new();
        let mut sink = |ev: &Event| lines.push(serde_json::to_string(ev).unwrap());
        let mut inf = ScriptedInference::new(SCRIPT_SUCCESS);
        let driver = GoalDriver::start(GoalId(1), "wire check", 8, &mut sink);
        driver.run(&mut inf, &mut sink);
        for line in lines {
            let back: Event = serde_json::from_str(&line).unwrap();
            assert!(matches!(back, Event::GoalStateChanged { .. }));
            let v: serde_json::Value = serde_json::from_str(&line).unwrap();
            assert_eq!(v["type"], "goal_state_changed");
        }
    }
}
