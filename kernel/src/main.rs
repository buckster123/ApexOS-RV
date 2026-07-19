#![no_std]
#![no_main]

extern crate alloc;

pub mod heap;
pub mod qemu;
pub mod uart;

use riscv_rt::entry;

#[entry]
fn main() -> ! {
    println!("apexos-rv: hart 0 online");
    heap::init();

    // P4.3 alloc smoke: format! → Vec → print → drop (deterministic output).
    {
        let s = alloc::format!("alloc ok: {}", 42);
        let mut v = alloc::vec::Vec::new();
        v.push(s);
        println!("{} (heap: {} KiB)", v[0], heap::SIZE / 1024);
    }

    // P4 negative gate: an allocation the tiny heap cannot serve → visible OOM.
    #[cfg(feature = "tiny-heap")]
    {
        let big = alloc::vec![0u8; 4 * 1024];
        println!("unreachable under tiny-heap: {}", big.len());
    }

    // P3.5 negative-path proof — inert without the feature (cfg-gated = disabled).
    #[cfg(feature = "panic-test")]
    panic!("panic-test: intentional panic to prove the reporting path");

    #[cfg(not(feature = "panic-test"))]
    qemu::exit_pass()
}

#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    println!("KERNEL PANIC: {info}");
    qemu::exit_fail(1)
}
