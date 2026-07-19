#![no_std]
#![no_main]

extern crate alloc;

pub mod agent;
pub mod heap;
pub mod net;
pub mod proto;
pub mod qemu;
pub mod time;
pub mod uart;

use riscv_rt::entry;

#[entry]
fn main() -> ! {
    println!("apexos-rv: hart 0 online");
    heap::init();
    time::init();
    let _nic = net::init(); // P9 banner; P10+ threads the device into the loop

    // P10 net-smoke gate: TCP echo against the host mock — diverges.
    #[cfg(feature = "net-smoke")]
    net::smoke::run(_nic);

    // P11 mesh-smoke gate: ws handshake + session_init — diverges.
    #[cfg(feature = "mesh-smoke")]
    net::mesh::smoke(_nic);

    #[cfg(not(any(feature = "net-smoke", feature = "mesh-smoke")))]
    normal_flow(_nic)
}

#[cfg(not(any(feature = "net-smoke", feature = "mesh-smoke")))]
fn normal_flow(nic: Option<net::Nic>) -> ! {
    // P4.3 alloc smoke: format! → Vec → print → drop (deterministic output).
    {
        let s = alloc::format!("alloc ok: {}", 42);
        let mut v = alloc::vec::Vec::new();
        v.push(s);
        println!("{} (heap: {} KiB)", v[0], heap::SIZE / 1024);
    }

    // P5.4 wire contract on metal: colony events round-trip and stream as JSON.
    proto::run();

    // P4 negative gate: an allocation the tiny heap cannot serve → visible OOM.
    #[cfg(feature = "tiny-heap")]
    {
        let big = alloc::vec![0u8; 4 * 1024];
        println!("unreachable under tiny-heap: {}", big.len());
    }

    // P3.5 negative-path proof — inert without the feature (cfg-gated = disabled).
    #[cfg(feature = "panic-test")]
    {
        let _ = nic;
        panic!("panic-test: intentional panic to prove the reporting path");
    }

    // P6 — colony state fold (asserted) + the scripted goal run, narrated in
    // protocol events; the goal's terminal state decides the exit code.
    #[cfg(not(feature = "panic-test"))]
    {
        if agent::run(nic) {
            qemu::exit_pass()
        } else {
            qemu::exit_fail(2)
        }
    }
}

#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    println!("KERNEL PANIC: {info}");
    qemu::exit_fail(1)
}
