//! SiFive test device on QEMU virt — turns kernel outcomes into QEMU exit codes,
//! which makes every phase gate a `$?` check (PRD D8).

const SIFIVE_TEST: *mut u32 = 0x0010_0000 as *mut u32;

const FINISHER_PASS: u32 = 0x5555;
const FINISHER_FAIL: u32 = 0x3333;

pub fn exit_pass() -> ! {
    // SAFETY: sifive_test MMIO on QEMU virt; 0x5555 = FINISHER_PASS → exit(0).
    unsafe { SIFIVE_TEST.write_volatile(FINISHER_PASS) };
    loop {
        riscv::asm::wfi();
    }
}

pub fn exit_fail(code: u16) -> ! {
    // SAFETY: as above; (code << 16) | 0x3333 = FINISHER_FAIL → exit(code).
    unsafe { SIFIVE_TEST.write_volatile(((code as u32) << 16) | FINISHER_FAIL) };
    loop {
        riscv::asm::wfi();
    }
}
