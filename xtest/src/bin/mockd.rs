//! Host-side mock peer for the mesh gates (PRD v2 D13). Modes grow with the
//! phases: echo (P10) → ws (P11) → llm/silent (P12). Deliberately boring:
//! fixed responses, one connection, deterministic output.

use std::io::{Read, Write};
use std::net::TcpListener;

fn main() {
    let mode = std::env::args().nth(1).unwrap_or_else(|| "echo".to_string());
    let port: u16 = std::env::args()
        .nth(2)
        .and_then(|p| p.parse().ok())
        .unwrap_or(9601);
    match mode.as_str() {
        "echo" => echo(port),
        "ws" => ws(port),
        "llm" => llm(port, false),
        "silent" => llm(port, true),
        other => {
            eprintln!("mockd: unknown mode {other}");
            std::process::exit(2);
        }
    }
}

/// Scripted-LLM gateway (P12, D13): session_init, then for each `user_prompt`
/// reply with two `agent_text` deltas + `turn_complete`; turn 3 verdicts
/// `done`. Contract asserts: the client's frame must omit `session` (the
/// gateway injects it) and `images`. In `silent` mode the first turn gets one
/// delta and then nothing — food for the kernel's mtime watchdog.
fn llm(port: u16, silent: bool) {
    let listener = TcpListener::bind(("127.0.0.1", port)).expect("mockd bind");
    println!("mockd: {} listening on {port}", if silent { "silent" } else { "llm" });
    let (stream, peer) = listener.accept().expect("mockd accept");
    println!("mockd: peer {peer}");
    let mut sock = tungstenite::accept(stream).expect("mockd ws accept");
    let send = |sock: &mut tungstenite::WebSocket<std::net::TcpStream>, s: String| {
        sock.send(tungstenite::Message::Text(s.into())).expect("mockd send");
    };
    send(&mut sock, r#"{"type": "session_init", "session_id": 42, "history": []}"#.into());
    println!("mockd: session_init sent");
    let mut turn = 0u32;
    loop {
        let msg = match sock.read() {
            Ok(m) if m.is_close() => break,
            Ok(m) => m,
            Err(_) => break,
        };
        let Ok(text) = msg.into_text() else { continue };
        let v: serde_json::Value = serde_json::from_str(&text).expect("mockd: json frame");
        assert_eq!(v["type"], "user_prompt", "mockd: expected user_prompt");
        assert!(v.get("session").is_none(), "mockd: client must omit session");
        assert!(v.get("images").is_none(), "mockd: metal sends no images");
        turn += 1;
        println!("mockd: turn {turn} prompt received");
        if silent {
            send(
                &mut sock,
                serde_json::json!({"type": "agent_text", "session": 42,
                    "delta": "thinking very hard about"})
                .to_string(),
            );
            println!("mockd: going silent (watchdog food)");
            continue;
        }
        let (body, verdict) = if turn < 3 {
            ("acknowledged — proceeding.", "GOAL_STEP: continue")
        } else {
            ("objective complete.", "GOAL_STEP: done")
        };
        send(
            &mut sock,
            serde_json::json!({"type": "agent_text", "session": 42,
                "delta": format!("Step {turn} {body}\n")})
            .to_string(),
        );
        send(
            &mut sock,
            serde_json::json!({"type": "agent_text", "session": 42, "delta": verdict})
                .to_string(),
        );
        send(&mut sock, serde_json::json!({"type": "turn_complete", "session": 42}).to_string());
    }
    println!("mockd: connection closed");
}

/// One connection; speak the gateway contract's opening: accept the upgrade,
/// push `session_init` (the gateway talks first), then serve until close.
fn ws(port: u16) {
    let listener = TcpListener::bind(("127.0.0.1", port)).expect("mockd bind");
    println!("mockd: ws listening on {port}");
    let (stream, peer) = listener.accept().expect("mockd accept");
    println!("mockd: peer {peer}");
    let mut sock = tungstenite::accept(stream).expect("mockd ws accept");
    sock.send(tungstenite::Message::Text(
        r#"{"type": "session_init", "session_id": 42, "history": []}"#.into(),
    ))
    .expect("mockd send session_init");
    println!("mockd: session_init sent");
    loop {
        match sock.read() {
            Ok(m) if m.is_close() => break,
            Ok(_) => {}
            Err(_) => break,
        }
    }
    println!("mockd: connection closed");
}

/// One connection; echo bytes until the peer closes.
fn echo(port: u16) {
    let listener = TcpListener::bind(("127.0.0.1", port)).expect("mockd bind");
    println!("mockd: echo listening on {port}");
    let (mut stream, peer) = listener.accept().expect("mockd accept");
    println!("mockd: peer {peer}");
    let mut buf = [0u8; 2048];
    loop {
        match stream.read(&mut buf) {
            Ok(0) | Err(_) => break,
            Ok(n) => {
                if stream.write_all(&buf[..n]).is_err() {
                    break;
                }
            }
        }
    }
    println!("mockd: connection closed");
}
