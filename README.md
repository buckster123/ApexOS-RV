# ApexOS-RV

**A bare-metal RISC-V node class for the [ApexOS](https://github.com/buckster123/ApexOS-RS) colony** — a `#![no_std]` Rust kernel for `riscv64gc-unknown-none-elf` where the agent runtime *is* the firmware: no Linux, no libc, boots on QEMU `virt` in milliseconds, speaks the colony's `apexos-protocol` wire contract natively, and runs the goal-driver semantics on bare metal.

Status: **docs graduated, Phase 0 in progress** — see [`PLAN.md`](PLAN.md) for live checklists, [`PRD.md`](PRD.md) for the full intent, [`CLAUDE.md`](CLAUDE.md) for the working rules.

```bash
# prerequisites (Phase 0): rustup + qemu-system-misc + gdb-multiarch
cargo run          # build the kernel and boot it in QEMU (Ctrl-A X exits)
cargo hosttest     # host-side tests incl. the cross-repo wire-compat check
```

Shared pieces (`apexos-protocol`, `state.rs`) are vendored from ApexOS-RS at a pinned commit with full provenance; the kernel's emitted event stream is proven parseable by the *unmodified upstream* crate. Upstream is never touched.

Apache-2.0. Vendored portions © upstream ApexOS-RS (same author).
