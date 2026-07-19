//! P11 — WebSocket client for the gateway `/ws` contract (upstream CLAUDE.md
//! §agentd WebSocket protocol): TCP → RFC 6455 handshake → `session_init`.
//! Machinery reused by P12's MeshInference; the `mesh-smoke` gate proves the
//! session dance end to end against the mock. Deterministic throughout (D12):
//! fixed smoltcp seed, fixed masking-RNG seed. Failures panic — the reporting
//! handler makes them visible and nonzero, which is what the gates key on.

use alloc::format;
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;

use apexos_rv_agent_core::gateway::GatewayControl;
use embedded_websocket::{
    Error as WsError, WebSocketClient, WebSocketCloseStatusCode, WebSocketOptions,
    WebSocketReceiveMessageType, WebSocketSendMessageType,
};
use rand_core::RngCore;
use smoltcp::iface::{Config, Interface, SocketHandle, SocketSet};
use smoltcp::socket::tcp;
use smoltcp::time::Instant;
use smoltcp::wire::{EthernetAddress, HardwareAddress, IpAddress, IpCidr, Ipv4Address};

use crate::net::smol::SmolNic;
use crate::net::{Nic, MAC};
use crate::{println, time};

pub const GATEWAY_PORT: u16 = 9601;
const LOCAL_PORT: u16 = 49153;
const ESTABLISH_TIMEOUT: u64 = 10 * time::TICKS_PER_SEC;
/// Optional build-time bearer token for the live colony (`APEXRV_TOKEN=… cargo build`).
const TOKEN: Option<&str> = option_env!("APEXRV_TOKEN");
/// Build-time gateway override for the live demo (defaults = the mock world).
const GATEWAY_HOST: &str = match option_env!("APEXRV_GATEWAY_IP") {
    Some(s) => s,
    None => "10.0.2.2",
};

fn gateway_addr() -> (Ipv4Address, u16) {
    let ip = match option_env!("APEXRV_GATEWAY_IP") {
        Some(s) => {
            let mut o = [0u8; 4];
            let mut it = s.split('.');
            for slot in &mut o {
                *slot = it
                    .next()
                    .and_then(|p| p.parse().ok())
                    .expect("APEXRV_GATEWAY_IP: dotted quad");
            }
            Ipv4Address::new(o[0], o[1], o[2], o[3])
        }
        None => Ipv4Address::new(10, 0, 2, 2),
    };
    let port = option_env!("APEXRV_GATEWAY_PORT")
        .and_then(|s| s.parse().ok())
        .unwrap_or(GATEWAY_PORT);
    (ip, port)
}

fn now() -> Instant {
    Instant::from_millis((time::mtime() / (time::TICKS_PER_SEC / 1000)) as i64)
}

/// Deterministic xorshift64* masking RNG. RFC 6455 masking needs no crypto
/// strength (it defeats proxy cache poisoning), and determinism is doctrine.
pub struct DetRng(u64);

impl DetRng {
    pub const fn new(seed: u64) -> Self {
        Self(seed)
    }
}

impl RngCore for DetRng {
    fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
    fn next_u32(&mut self) -> u32 {
        (self.next_u64() >> 32) as u32
    }
    fn fill_bytes(&mut self, dest: &mut [u8]) {
        for chunk in dest.chunks_mut(8) {
            let b = self.next_u64().to_le_bytes();
            chunk.copy_from_slice(&b[..chunk.len()]);
        }
    }
    fn try_fill_bytes(&mut self, dest: &mut [u8]) -> Result<(), rand_core::Error> {
        self.fill_bytes(dest);
        Ok(())
    }
}

pub struct Mesh {
    dev: SmolNic,
    iface: Interface,
    sockets: SocketSet<'static>,
    handle: SocketHandle,
    ws: WebSocketClient<DetRng>,
    /// TCP bytes received but not yet consumed by the ws decoder.
    rx_raw: Vec<u8>,
    /// Partial text message accumulated across fragmented ws frames.
    msg_partial: Vec<u8>,
    pub session_id: u64,
}

impl Mesh {
    /// TCP connect → ws upgrade → first `session_init`. Panics on failure.
    /// The gateway address comes from the D12 defaults or the build-time
    /// `APEXRV_GATEWAY_IP`/`APEXRV_GATEWAY_PORT` overrides (live demo).
    pub fn establish(nic: Nic) -> Mesh {
        let (gw_ip, gw_port) = gateway_addr();
        let mut dev = SmolNic::new(nic);
        let mut config = Config::new(HardwareAddress::Ethernet(EthernetAddress(MAC)));
        config.random_seed = 0xA9E0_5117; // deterministic ISNs (D12)
        let mut iface = Interface::new(config, &mut dev, now());
        iface.update_ip_addrs(|addrs| {
            addrs.push(IpCidr::new(IpAddress::v4(10, 0, 2, 15), 24)).unwrap();
        });
        iface
            .routes_mut()
            .add_default_ipv4_route(Ipv4Address::new(10, 0, 2, 2))
            .unwrap();

        let mut sockets = SocketSet::new(vec![]);
        let handle = sockets.add(tcp::Socket::new(
            tcp::SocketBuffer::new(vec![0u8; 8192]),
            tcp::SocketBuffer::new(vec![0u8; 8192]),
        ));
        sockets
            .get_mut::<tcp::Socket>(handle)
            .connect(iface.context(), (IpAddress::Ipv4(gw_ip), gw_port), LOCAL_PORT)
            .expect("mesh: tcp connect setup");

        let mut ws = WebSocketClient::new_client(DetRng::new(0x5EED_CAFE_D00D));
        let auth = TOKEN.map(|t| format!("Authorization: Bearer {t}"));
        let hdr_store;
        let extra_headers = match auth.as_deref() {
            Some(h) => {
                hdr_store = [h];
                Some(&hdr_store[..])
            }
            None => None,
        };
        let opts = WebSocketOptions {
            path: "/ws",
            host: GATEWAY_HOST,
            origin: "",
            sub_protocols: None,
            additional_headers: extra_headers,
        };
        let mut hs = vec![0u8; 2048];
        let (len, key) = ws.client_connect(&opts, &mut hs).expect("mesh: build upgrade");

        let mut mesh = Mesh {
            dev,
            iface,
            sockets,
            handle,
            ws,
            rx_raw: Vec::new(),
            msg_partial: Vec::new(),
            session_id: 0,
        };
        let upgrade = hs[..len].to_vec();
        mesh.blocking_send_raw(&upgrade);

        // Handshake response: accumulate until client_accept stops asking for more.
        let deadline = time::mtime() + ESTABLISH_TIMEOUT;
        loop {
            mesh.pump();
            match mesh.ws.client_accept(&key, &mesh.rx_raw) {
                Ok((consumed, _sub)) => {
                    mesh.rx_raw.drain(..consumed);
                    break;
                }
                Err(WsError::HttpHeaderIncomplete) => {}
                Err(e) => panic!("mesh: handshake rejected: {e:?}"),
            }
            mesh.idle_tick(deadline, "ws handshake");
        }

        // The gateway speaks first: the session frame.
        let text = mesh.next_text(deadline);
        let GatewayControl::SessionInit { session_id, .. } =
            serde_json::from_str(&text).expect("mesh: session_init parse");
        mesh.session_id = session_id;
        mesh
    }

    /// Poll the interface and drain every received TCP byte into `rx_raw`.
    fn pump(&mut self) {
        self.iface.poll(now(), &mut self.dev, &mut self.sockets);
        let sock = self.sockets.get_mut::<tcp::Socket>(self.handle);
        while sock.can_recv() {
            let _ = sock.recv(|b| {
                self.rx_raw.extend_from_slice(b);
                (b.len(), ())
            });
        }
    }

    fn idle_tick(&mut self, deadline: u64, what: &str) {
        if time::mtime() > deadline {
            panic!("mesh: timed out during {what}");
        }
        time::arm_wakeup(time::mtime() + time::TICKS_PER_SEC / 1000);
        riscv::asm::wfi();
    }

    /// Push raw bytes through the TCP socket until fully queued + flushed.
    fn blocking_send_raw(&mut self, bytes: &[u8]) {
        let deadline = time::mtime() + ESTABLISH_TIMEOUT;
        let mut off = 0;
        while off < bytes.len() {
            self.iface.poll(now(), &mut self.dev, &mut self.sockets);
            let sock = self.sockets.get_mut::<tcp::Socket>(self.handle);
            if sock.can_send() {
                off += sock.send_slice(&bytes[off..]).expect("mesh: tcp send");
            }
            if off < bytes.len() {
                self.idle_tick(deadline, "tcp send");
            }
        }
        self.iface.poll(now(), &mut self.dev, &mut self.sockets);
    }

    /// Send one ws text frame.
    pub fn send_text(&mut self, payload: &str) {
        let mut out = vec![0u8; payload.len() + 64];
        let n = self
            .ws
            .write(WebSocketSendMessageType::Text, true, payload.as_bytes(), &mut out)
            .expect("mesh: ws write");
        let framed = out[..n].to_vec();
        self.blocking_send_raw(&framed);
    }

    /// Non-blocking: pump once and return the next complete ws text message if
    /// one is decodable now (answers pings en route; accumulates fragments in
    /// `msg_partial` across calls).
    pub fn try_next_text(&mut self) -> Option<String> {
        self.pump();
        let mut frame = vec![0u8; 4096];
        while !self.rx_raw.is_empty() {
            match self.ws.read(&self.rx_raw, &mut frame) {
                Ok(r) => {
                    self.rx_raw.drain(..r.len_from);
                    match r.message_type {
                        WebSocketReceiveMessageType::Text => {
                            self.msg_partial.extend_from_slice(&frame[..r.len_to]);
                            if r.end_of_message {
                                let msg = core::mem::take(&mut self.msg_partial);
                                return Some(String::from_utf8(msg).expect("mesh: utf8"));
                            }
                        }
                        WebSocketReceiveMessageType::Ping => {
                            let mut pong = vec![0u8; r.len_to + 16];
                            let n = self
                                .ws
                                .write(
                                    WebSocketSendMessageType::Pong,
                                    true,
                                    &frame[..r.len_to],
                                    &mut pong,
                                )
                                .expect("mesh: pong");
                            let reply = pong[..n].to_vec();
                            self.blocking_send_raw(&reply);
                        }
                        WebSocketReceiveMessageType::CloseMustReply
                        | WebSocketReceiveMessageType::CloseCompleted => {
                            panic!("mesh: peer closed mid-session")
                        }
                        _ => {} // Binary/Pong: ignored in v2
                    }
                }
                Err(WsError::ReadFrameIncomplete) => return None,
                Err(e) => panic!("mesh: ws read: {e:?}"),
            }
        }
        None
    }

    /// Block until the next complete ws text message.
    pub fn next_text(&mut self, deadline: u64) -> String {
        loop {
            if let Some(msg) = self.try_next_text() {
                return msg;
            }
            self.idle_tick(deadline, "awaiting ws frame");
        }
    }

    /// Initiate a clean close; best-effort completion, never fails the gate.
    pub fn close_clean(&mut self) {
        let mut out = vec![0u8; 128];
        if let Ok(n) = self.ws.close(WebSocketCloseStatusCode::NormalClosure, None, &mut out) {
            let frame = out[..n].to_vec();
            self.blocking_send_raw(&frame);
        }
        let deadline = time::mtime() + 2 * time::TICKS_PER_SEC;
        let mut frame = vec![0u8; 512];
        while time::mtime() < deadline {
            self.pump();
            if !self.rx_raw.is_empty() {
                if let Ok(r) = self.ws.read(&self.rx_raw, &mut frame) {
                    self.rx_raw.drain(..r.len_from);
                    if matches!(r.message_type, WebSocketReceiveMessageType::CloseCompleted) {
                        break;
                    }
                    continue;
                }
            }
            time::arm_wakeup(time::mtime() + time::TICKS_PER_SEC / 1000);
            riscv::asm::wfi();
        }
        self.sockets.get_mut::<tcp::Socket>(self.handle).close();
        for _ in 0..10 {
            self.iface.poll(now(), &mut self.dev, &mut self.sockets);
            time::arm_wakeup(time::mtime() + time::TICKS_PER_SEC / 1000);
            riscv::asm::wfi();
        }
    }
}

/// P11 gate: establish the session, announce it, close, exit 0.
#[cfg(feature = "mesh-smoke")]
pub fn smoke(nic: Option<Nic>) -> ! {
    let nic = nic.expect("mesh-smoke requires the NIC");
    let mut mesh = Mesh::establish(nic);
    println!("mesh: session {} established", mesh.session_id);
    mesh.close_clean();
    crate::qemu::exit_pass()
}

// ── P12: MeshInference — goal turns over the gateway wire ────────────────────

/// Each goal step is one gateway turn: `user_prompt` out (the D14 directive),
/// `agent_text` deltas accumulate, `turn_complete` yields the verdict via the
/// trailing `GOAL_STEP:` line (absent → `Continue(None)`, upstream's default).
/// While the turn is in flight `next` returns `Pending` — the P7 mtime
/// watchdog guards the wire.
#[cfg(feature = "mesh-goal")]
pub struct MeshInference {
    mesh: Mesh,
    reply: String,
    in_flight: bool,
}

#[cfg(feature = "mesh-goal")]
impl MeshInference {
    pub fn new(mesh: Mesh) -> Self {
        Self { mesh, reply: String::new(), in_flight: false }
    }

    /// Hand the session back (for a clean close after the goal ends).
    pub fn finish(self) -> Mesh {
        self.mesh
    }
}

#[cfg(feature = "mesh-goal")]
impl apexos_rv_agent_core::Inference for MeshInference {
    fn next(
        &mut self,
        ctx: &apexos_rv_agent_core::TickContext<'_>,
    ) -> apexos_rv_agent_core::TurnResult {
        use apexos_rv_agent_core::gateway::ClientFrame;
        use apexos_rv_agent_core::{goal, TurnResult, Verdict};

        if !self.in_flight {
            let frame = ClientFrame::UserPrompt { text: goal::step_directive(ctx) };
            let json = serde_json::to_string(&frame).expect("mesh: frame serialize");
            self.mesh.send_text(&json);
            self.in_flight = true;
            self.reply.clear();
            return TurnResult::Pending;
        }
        while let Some(text) = self.mesh.try_next_text() {
            match serde_json::from_str::<apexos_protocol::Event>(&text) {
                Ok(apexos_protocol::Event::AgentText { delta, .. }) => {
                    self.reply.push_str(&delta);
                }
                Ok(apexos_protocol::Event::TurnComplete { .. }) => {
                    self.in_flight = false;
                    let verdict =
                        goal::parse_goal_step(&self.reply).unwrap_or(Verdict::Continue(None));
                    return TurnResult::Completed(verdict);
                }
                Ok(_) => {}  // other session/global events: ignored in v2
                Err(_) => {} // control frames (e.g. a re-pushed session_init)
            }
        }
        TurnResult::Pending
    }
}
