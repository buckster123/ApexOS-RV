//! P6 kernel integration — the colony's state fold and a scripted goal run,
//! narrated over UART one JSON event per line. The kernel owns I/O only;
//! all logic lives in `apexos-rv-agent-core` (host-tested).

use alloc::string::ToString;
use alloc::vec;
use apexos_protocol::{ActionId, Event, GoalId, GoalState, PluginId, SessionId, ToolCall, ToolSpec};
use apexos_rv_agent_core::{GoalDriver, ScriptedInference};

use crate::println;

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

/// Run the fold demo then drive the scripted goal to its terminal state.
/// Returns `true` iff the goal ended `Done`.
pub fn run() -> bool {
    fold_demo();

    #[cfg(not(feature = "fail-script"))]
    let script = apexos_rv_agent_core::SCRIPT_SUCCESS;
    #[cfg(feature = "fail-script")]
    let script = apexos_rv_agent_core::SCRIPT_STALL;

    let mut inf = ScriptedInference::new(script);
    let driver = GoalDriver::start(
        GoalId(1),
        "prove the colony contract end-to-end on bare metal",
        8,
        &mut emit,
    );
    let terminal = driver.run(&mut inf, &mut emit);
    matches!(terminal, GoalState::Done)
}
