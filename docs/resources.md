# Resources — curated references for ApexOS-RV

One-line *why* per link. Add sparingly; this is a working set, not a directory.

## RISC-V specs & profiles

- **ISA manuals (unprivileged + privileged)** — https://github.com/riscv/riscv-isa-manual — the privileged spec is where `mcause`/`mepc`/`mtvec`/`mstatus` live (Phase 3 trap printing, Appendix C debugging).
- **Profiles (RVA23 etc.)** — https://github.com/riscv/riscv-profiles — what "application-class RISC-V" guarantees; our `rv64gc` target is the RVA20-ish baseline, RVA23 (V, hypervisor, …) matters when real silicon arrives.
- **SBI spec** — https://github.com/riscv-non-isa/riscv-sbi-doc — not used in v1 (`-bios none`, M-mode), required reading for the post-v1 OpenSBI/S-mode path (PRD D2 alternative).

## QEMU `virt`

- **Machine docs** — https://www.qemu.org/docs/master/system/riscv/virt.html — canonical description of what we boot on.
- **Memory map ground truth**: dump the device tree QEMU actually builds:
  `qemu-system-riscv64 -machine virt,dumpdtb=virt.dtb -smp 1 -m 128M` then `dtc -I dtb -O dts virt.dtb` (needs `device-tree-compiler`). Settles any "is the UART really at 0x1000_0000" doubt empirically.
- **Ubuntu packaging note (2026)**: riscv64 emulation moved out of `qemu-system-misc` into its own **`qemu-system-riscv`** package — old install guides silently miss it.

## Crates (read docs *at the pinned version* — PLAN rule 6)

- **riscv-rt** — https://docs.rs/riscv-rt — boot/runtime; 0.18's trap + hart-parking surface differs from the 0.12-era tutorials everywhere online.
- **riscv** — https://docs.rs/riscv — CSR access, `wfi`, the single-hart `critical-section` impl (PRD D5).
- **embedded-alloc** — https://docs.rs/embedded-alloc — the `#[global_allocator]` (Phase 4).
- **critical-section** — https://docs.rs/critical-section — the abstraction the allocator + our print lock sit on; understand it before ever raising `-smp`.
- All live under https://github.com/rust-embedded — the rust-embedded WG repos are the upstream for issues/examples.

## `no_std` foundations

- **The Embedonomicon** — https://docs.rust-embedded.org/embedonomicon/ — builds a `#![no_std]` program from nothing; the deep explanation of what riscv-rt does for us.
- **The Embedded Rust Book** — https://docs.rust-embedded.org/book/ — broader patterns (peripherals as types, `unsafe` hygiene).

## RISC-V OS-dev references (concepts, not dependencies)

- **"The Adventures of OS"** (Stephen Marz) — https://osblog.stephenmarz.com/ — RISC-V OS in Rust, chapter-by-chapter; best prose on UART/traps/CLINT.
- **xv6-riscv** — https://github.com/mit-pdos/xv6-riscv — the reference teaching kernel; its `kernel/memlayout.h` mirrors our MMIO map.
- **OSDev wiki** — https://wiki.osdev.org/RISC-V and https://wiki.osdev.org/Serial_Ports (16550 UART register map).

## Upstream ApexOS-RS (the pin: `676aa3870ad7`)

- Repo — https://github.com/buckster123/ApexOS-RS — read `AGENTS.md` (the load-bearing-boundary doctrine) and `CLAUDE.md` first.
- **Goal-driver design doc** — vendored at [`docs/upstream/goal-driver-design.md`](upstream/goal-driver-design.md) (provenance header inside); shipped `goal.rs` wins on divergence.
- **Wire-format ground truth** — `apexos-protocol/src/lib.rs` `#[cfg(test)]` block: the exact JSON strings frontends deserialize; our Phase 5 wire-compat tests mirror its style.

## Host tooling

- **gdb-multiarch** — installed; recipes in PLAN Appendix C.
- **cargo-binutils** (`cargo install cargo-binutils` + `rustup component add llvm-tools`) — `rust-objdump`/`rust-size`/`rust-objcopy` on the kernel ELF; `objcopy -O binary` becomes load-bearing at real-board time.
- **device-tree-compiler** (`dtc`) — for the dumpdtb trick above; optional.
