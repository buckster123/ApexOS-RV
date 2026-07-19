//! Pure agent logic for ApexOS-RV: the colony's `SystemState` event-fold
//! (SYNC-COPY of upstream `state.rs`) and, from P6.2, a fresh `no_std` goal
//! driver mirroring upstream `goal.rs` semantics. No I/O lives here — the
//! kernel owns MMIO; host tests own the harness (CLAUDE.md rule 3 boundary).

#![cfg_attr(not(test), no_std)]

extern crate alloc;

pub mod goal;
pub mod state;

pub use goal::{
    GoalDriver, Inference, ScriptStep, ScriptedInference, TickContext, TurnResult, Verdict,
    DEFAULT_MAX_STEPS, MAX_STEPS_CEIL, SCRIPT_HANG, SCRIPT_STALL, SCRIPT_SUCCESS,
};
pub use state::SystemState;
