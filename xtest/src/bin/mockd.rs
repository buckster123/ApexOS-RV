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
        other => {
            eprintln!("mockd: unknown mode {other}");
            std::process::exit(2);
        }
    }
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
