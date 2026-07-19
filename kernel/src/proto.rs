//! P5.4 in-kernel wire-contract round-trip — the risky no_std serialization
//! paths (f32/f64 floats, serde_json::Value trees) proven on metal, one JSON
//! event per line over UART (the colony's line-delimited contract).

use alloc::string::ToString;
use apexos_protocol::{ActionId, Event, SensorReading, SessionId, ToolCall};

// The kernel's print macros are #[macro_export]ed from uart.rs; a bare invocation
// here is outside their textual scope, so import them by path.
use crate::println;

/// Serialize → parse back → re-serialize; assert byte and value-tree stability;
/// print the wire line. Any failure panics → reporting handler → exit_fail.
fn roundtrip(ev: &Event) {
    let json = serde_json::to_string(ev).unwrap();
    let back: Event = serde_json::from_str(&json).unwrap();
    let json2 = serde_json::to_string(&back).unwrap();
    assert_eq!(json, json2, "re-serialization must be byte-identical");
    let v1: serde_json::Value = serde_json::from_str(&json).unwrap();
    let v2 = serde_json::to_value(&back).unwrap();
    assert_eq!(v1, v2, "value trees must match");
    println!("{json}");
}

pub fn run() {
    // f32 fields — the no_std float-formatting risk named in PRD §11.
    roundtrip(&Event::SensorReading {
        node_id: "rv-metal-0".to_string(),
        reading: SensorReading::AirQuality {
            iaq: 51.5,
            co2_eq_ppm: 407.0,
            voc_ppm: 0.5,
            accuracy: 3,
            temperature_c: 22.5,
            humidity_pct: 40.25,
            pressure_hpa: 1013.25,
            sensor_id: "bme688".to_string(),
        },
        timestamp: 1_777_000_000,
    });

    // f64 field (and a non-dyadic value — the honest ryu round-trip test).
    roundtrip(&Event::VastInstanceLaunched {
        instance_id: "vast-31337".to_string(),
        recipe: "qwen3-30b-a3b".to_string(),
        cost_per_hr: 0.297,
    });

    // serde_json::Value tree (ToolCall.args).
    roundtrip(&Event::ToolRequested {
        session: SessionId(1),
        call: ToolCall {
            id: ActionId(7),
            tool: "cerebro.recall".to_string(),
            args: serde_json::json!({ "query": "hart 0 online", "k": 3 }),
            needs_approval: false,
        },
    });

    println!("proto: 3 events round-tripped on metal");
}
