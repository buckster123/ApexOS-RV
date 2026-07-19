//! Minimal NS16550A driver for QEMU virt — polled TX only (RX is post-v1).
//!
//! QEMU's model comes up ready to transmit, so there is no init here.
//! real hardware: init clocks/divisor (LCR/DLL/DLM) before the first write.

use core::fmt;

const UART0: *mut u8 = 0x1000_0000 as *mut u8; // NS16550A base on QEMU virt
const LSR: usize = 5; // line status register offset
const LSR_THRE: u8 = 1 << 5; // transmitter holding register empty

pub fn putb(b: u8) {
    // SAFETY: UART0 is the QEMU-virt NS16550A MMIO base; byte-wide volatile
    // access to THR (+0) and LSR (+5) is the device contract. Single hart (D4)
    // and print callers hold a critical section, so writes never interleave.
    unsafe {
        while UART0.add(LSR).read_volatile() & LSR_THRE == 0 {}
        UART0.write_volatile(b);
    }
}

pub struct Writer;

impl fmt::Write for Writer {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for b in s.bytes() {
            if b == b'\n' {
                putb(b'\r');
            }
            putb(b);
        }
        Ok(())
    }
}

#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => {{
        use core::fmt::Write as _;
        critical_section::with(|_| {
            let _ = write!($crate::uart::Writer, $($arg)*);
        });
    }};
}

#[macro_export]
macro_rules! println {
    () => { $crate::print!("\n") };
    ($($arg:tt)*) => {{
        use core::fmt::Write as _;
        critical_section::with(|_| {
            let _ = writeln!($crate::uart::Writer, $($arg)*);
        });
    }};
}
