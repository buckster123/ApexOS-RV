#![no_std]
#![no_main]

use riscv_rt::entry;

#[entry]
fn main() -> ! {
    loop {}
}

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    loop {}
}
