//! CLINT timer access on QEMU virt — the kernel's monotonic time source (P7).
//!
//! Polling only (PLAN P7.1): MIE stays off and no trap handler is installed.
//! `mtimecmp` is armed solely so `wfi` has a pending-interrupt wake condition —
//! tickless idle without interrupts (a pending-but-disabled MTIP wakes `wfi`
//! per the privileged spec; no trap is ever taken).

const CLINT_MTIME: *const u64 = 0x0200_BFF8 as *const u64;
const CLINT_MTIMECMP0: *mut u64 = 0x0200_4000 as *mut u64;

/// QEMU virt timebase: 10 MHz → 1 tick = 100 ns.
pub const TICKS_PER_SEC: u64 = 10_000_000;

/// One-time setup: per-source-enable the machine timer interrupt (`mie.MTIE`)
/// so a pending MTIP can wake `wfi`. riscv-rt boots with `mie = 0`, under
/// which `wfi` has no wake condition and sleeps forever. Global `mstatus.MIE`
/// stays off, so no trap is ever taken — this only gives idle a wake signal.
pub fn init() {
    // SAFETY: setting an interrupt-enable bit with mstatus.MIE clear cannot
    // cause a trap; single hart (D4), called once at boot.
    unsafe { riscv::register::mie::set_mtimer() };
}

pub fn mtime() -> u64 {
    // SAFETY: CLINT mtime MMIO on QEMU virt; an aligned u64 volatile read is
    // the device contract (rv64 performs it in a single access).
    unsafe { CLINT_MTIME.read_volatile() }
}

/// Arm hart 0's `mtimecmp` so MTIP pends at `deadline`, giving `wfi` a wake
/// condition. With MIE off this never traps — it is idle, not interrupts.
pub fn arm_wakeup(deadline: u64) {
    // SAFETY: CLINT mtimecmp0 MMIO on QEMU virt; single hart (D4) owns it.
    unsafe { CLINT_MTIMECMP0.write_volatile(deadline) };
}
