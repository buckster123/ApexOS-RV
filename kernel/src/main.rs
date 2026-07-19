#![no_std]
#![no_main]

pub mod qemu;
pub mod uart;

use riscv_rt::entry;

#[entry]
fn main() -> ! {
    println!("apexos-rv: hart 0 online");

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
