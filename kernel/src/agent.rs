//! P6 kernel integration — the colony's state fold and a scripted goal run,
//! narrated over UART one JSON event per line. The kernel owns I/O only;
//! all logic lives in `apexos-rv-agent-core` (host-tested).

use alloc::string::ToString;
use alloc::vec;
use apexos_protocol::{ActionId, Event, GoalId, GoalState, PluginId, SessionId, ToolCall, ToolSpec};
use apexos_rv_agent_core::{GoalDriver, ScriptedInference, TurnResult};

use crate::{println, time};

/// Scaled-down analog of upstream's 900 s `STEP_TIMEOUT` (goal.rs @ pin):
/// 2 s of emulated time — long enough to prove real waiting, short enough
/// for the 30 s harness timeout.
const STEP_TIMEOUT_TICKS: u64 = 2 * time::TICKS_PER_SEC;
/// Idle quantum between watchdog checks while a turn is pending: 10 ms.
const IDLE_QUANTUM_TICKS: u64 = time::TICKS_PER_SEC / 100;

fn emit(ev: &Event) {
    println!("{}", serde_json::to_string(ev).unwrap());
}

/// P6.5 — fold a scripted event sequence through the SYNC-COPY `SystemState`
/// and assert the colony's state semantics hold on metal (any failed assert
/// panics → reporting handler → nonzero exit).
fn fold_demo() {
    let mut st = apexos_rv_agent_core::SystemState::default();
    let seq = [
        Event::UserPrompt {
            session: SessionId(1),
            text: "bring the colony contract to metal".to_string(),
            images: vec![],
        },
        Event::PluginUp {
            plugin: PluginId("cerebro".to_string()),
            tools: vec![
                ToolSpec {
                    name: "cerebro.recall".to_string(),
                    description: "recall memories".to_string(),
                    input_schema: serde_json::json!({}),
                },
                ToolSpec {
                    name: "cerebro.store".to_string(),
                    description: "store a memory".to_string(),
                    input_schema: serde_json::json!({}),
                },
            ],
        },
        Event::ApprovalPending {
            session: SessionId(1),
            call: ToolCall {
                id: ActionId(7),
                tool: "cerebro.store".to_string(),
                args: serde_json::json!({ "content": "hart 0 online" }),
                needs_approval: true,
            },
        },
        Event::UserApproval { session: SessionId(1), action: ActionId(7), granted: true },
    ];
    for ev in &seq {
        emit(ev);
        st.apply(ev);
    }
    assert!(st.sessions.get(&SessionId(1)).is_some_and(|c| c.is_root() && c.history.len() == 1));
    assert_eq!(st.tools.len(), 2);
    assert!(st.pending_approvals.is_empty());
    println!("state: fold ok — sessions=1 tools=2 approvals=0");
}

/// Run the fold demo, then drive the scripted goal through the P7 cooperative
/// loop: poll → armed-`wfi` idle → mtime stall watchdog (upstream
/// `fail_stalled` semantics, measured in real emulated time). Returns `true`
/// iff the goal ended `Done`.
pub fn run() -> bool {
    fold_demo();

    #[cfg(not(feature = "fail-script"))]
    let script = apexos_rv_agent_core::SCRIPT_SUCCESS;
    #[cfg(feature = "fail-script")]
    let script = apexos_rv_agent_core::SCRIPT_HANG;

    let mut inf = ScriptedInference::new(script);
    let mut driver = GoalDriver::start(
        GoalId(1),
        "prove the colony contract end-to-end on bare metal",
        8,
        &mut emit,
    );

    let mut polls: u64 = 0;
    let mut last_step = driver.step();
    let mut step_started = time::mtime();
    while matches!(driver.state(), GoalState::Acting) {
        polls += 1;
        driver.poll(&mut inf, &mut emit);
        if driver.step() != last_step {
            last_step = driver.step();
            step_started = time::mtime();
        }
        if matches!(driver.state(), GoalState::Acting) {
            let now = time::mtime();
            if now.wrapping_sub(step_started) > STEP_TIMEOUT_TICKS {
                // The watchdog, not the LLM, fails a quiet step (fail_stalled).
                driver.advance(TurnResult::Stalled, &mut emit);
            } else {
                time::arm_wakeup(now + IDLE_QUANTUM_TICKS);
                riscv::asm::wfi();
            }
        }
    }

    if matches!(driver.state(), GoalState::Done) {
        // Poll count is deterministic on the success path (every poll
        // transitions); the fail path prints nothing timing-derived.
        println!("loop: goal reached Done in {polls} polls");
        println!("APEXOS-RV: goal done — halting");
        true
    } else {
        false
    }
}
