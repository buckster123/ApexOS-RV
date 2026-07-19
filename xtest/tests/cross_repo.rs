//! The G7 crown jewel: metal's captured UART stream deserialized by the
//! **pristine upstream** `apexos-protocol` (git dependency @ the audited pin,
//! std, unmodified) — wire compatibility proven across both the std/no_std
//! divide and the repo boundary. The fixture is a checked-in `run-qemu.sh`
//! capture of the release kernel (CRLF line endings and all).

use upstream_protocol::{Event, GoalState};

const FIXTURE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/success-run.log");

fn frames() -> Vec<Event> {
    let log = std::fs::read_to_string(FIXTURE).expect("fixture present");
    log.lines()
        .map(str::trim_end)
        .filter(|l| l.starts_with('{'))
        .map(|l| {
            serde_json::from_str(l)
                .unwrap_or_else(|e| panic!("upstream rejected metal frame {l}: {e}"))
        })
        .collect()
}

#[test]
fn every_metal_json_line_parses_with_the_upstream_types() {
    let events = frames();
    assert_eq!(events.len(), 11, "3 proto round-trips + 4 fold + 4 goal frames");
}

#[test]
fn the_narrative_ends_with_the_goal_reaching_done() {
    match frames().last().expect("frames present") {
        Event::GoalStateChanged { state, step, max_steps, yolo, .. } => {
            assert_eq!(*state, GoalState::Done);
            assert_eq!(*step, 3);
            assert_eq!(*max_steps, 8);
            assert!(!yolo);
        }
        other => panic!("expected the Done frame last, got {other:?}"),
    }
}

#[test]
fn the_goal_walk_is_the_upstream_lifecycle_acting_then_done() {
    let states: Vec<(GoalState, u32)> = frames()
        .into_iter()
        .filter_map(|ev| match ev {
            Event::GoalStateChanged { state, step, .. } => Some((state, step)),
            _ => None,
        })
        .collect();
    assert_eq!(
        states,
        [
            (GoalState::Acting, 1),
            (GoalState::Acting, 2),
            (GoalState::Acting, 3),
            (GoalState::Done, 3),
        ]
    );
}
