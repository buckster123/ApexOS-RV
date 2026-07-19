//! P10 net-smoke: TCP echo against the host-side mock at 10.0.2.2 — proves the
//! virtio-net + smoltcp weave end to end, gated by exit code. Diverges.

use alloc::vec;
use smoltcp::iface::{Config, Interface, SocketSet};
use smoltcp::socket::tcp;
use smoltcp::time::Instant;
use smoltcp::wire::{EthernetAddress, HardwareAddress, IpAddress, IpCidr, Ipv4Address};

use crate::net::smol::SmolNic;
use crate::net::{Nic, MAC};
use crate::{println, qemu, time};

const MOCK_PORT: u16 = 9601;
const LOCAL_PORT: u16 = 49152;
const PING: &[u8] = b"apexos-rv: ping over tcp\n";
const TIMEOUT_TICKS: u64 = 10 * time::TICKS_PER_SEC;

fn now() -> Instant {
    Instant::from_millis((time::mtime() / (time::TICKS_PER_SEC / 1000)) as i64)
}

pub fn run(nic: Option<Nic>) -> ! {
    let nic = nic.expect("net-smoke requires the NIC");
    let mut dev = SmolNic::new(nic);

    let mut config = Config::new(HardwareAddress::Ethernet(EthernetAddress(MAC)));
    config.random_seed = 0xA9E0_5117; // fixed seed: deterministic ISNs (D12)
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
        tcp::SocketBuffer::new(vec![0u8; 4096]),
        tcp::SocketBuffer::new(vec![0u8; 4096]),
    ));
    sockets
        .get_mut::<tcp::Socket>(handle)
        .connect(iface.context(), (IpAddress::v4(10, 0, 2, 2), MOCK_PORT), LOCAL_PORT)
        .expect("tcp connect setup");

    let start = time::mtime();
    let mut sent = false;
    let mut echoed: usize = 0;
    let mut buf = [0u8; 64];
    loop {
        if time::mtime().wrapping_sub(start) > TIMEOUT_TICKS {
            panic!("net-smoke: timed out awaiting echo");
        }
        iface.poll(now(), &mut dev, &mut sockets);
        let sock = sockets.get_mut::<tcp::Socket>(handle);
        if !sent && sock.can_send() {
            sock.send_slice(PING).expect("send");
            sent = true;
        }
        if sent && sock.can_recv() {
            let n = sock.recv_slice(&mut buf[echoed..]).expect("recv");
            echoed += n;
            if echoed >= PING.len() {
                assert_eq!(&buf[..PING.len()], PING, "echo mismatch");
                sock.close();
                println!("net: tcp echo ok");
                qemu::exit_pass();
            }
        }
        // Brief armed idle between polls (1 ms) — same no-interrupt discipline.
        time::arm_wakeup(time::mtime() + time::TICKS_PER_SEC / 1000);
        riscv::asm::wfi();
    }
}
