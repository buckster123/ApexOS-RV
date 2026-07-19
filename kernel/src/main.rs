#![no_std]
#![no_main]

use riscv_rt::entry;

const UART0: *mut u8 = 0x1000_0000 as *mut u8; // NS16550A THR on QEMU virt

#[entry]
fn main() -> ! {
    // P2.1 crude poke — the real driver with LSR polling lands in Phase 3.
    for &b in b"apexos-rv: hart 0 online\r\n" {
        // SAFETY: QEMU-virt NS16550A THR MMIO; byte-wide volatile store is the
        // device contract. No LSR poll yet: QEMU's THR never stalls (P3 adds it).
        unsafe { UART0.write_volatile(b) };
    }
    loop {
        riscv::asm::wfi();
    }
}

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    loop {}
}
