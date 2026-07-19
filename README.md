# ApexOS-RV

**A bare-metal RISC-V node class for the [ApexOS](https://github.com/buckster123/ApexOS-RS) colony** — a `#![no_std]` Rust kernel for `riscv64gc-unknown-none-elf` where the agent runtime *is* the firmware: no Linux, no libc. It boots on QEMU `virt` in milliseconds, speaks the colony's `apexos-protocol` wire contract natively, folds events through the colony's own `SystemState`, and drives the goal lifecycle with upstream's exact semantics ("LLM-proposes / code-disposes") against scripted inference — the mesh client comes post-v1.

**Status: v1.0.0 — all eight phases gated.** [`PLAN.md`](PLAN.md) has the evidence-annotated checklists, [`PRD.md`](PRD.md) the full intent, [`CLAUDE.md`](CLAUDE.md) the working rules, [`BACKLOG.md`](BACKLOG.md) what v1 said no to.

```bash
# prerequisites: rustup + qemu (Ubuntu ≥25.10: qemu-system-riscv; older: qemu-system-misc)
cargo run --release     # build + boot in QEMU; the kernel reports its own exit code
cargo hosttest          # host tests incl. the cross-repo wire-compat check
./scripts/run-qemu.sh target/riscv64gc-unknown-none-elf/release/apexos-rv-kernel   # gated run
```

What a run looks like (deterministic — byte-identical across runs, by gate):

```text
apexos-rv: hart 0 online
alloc ok: 42 (heap: 1024 KiB)
{"type":"sensor_reading","node_id":"rv-metal-0","reading":{"kind":"air_quality",...}}
{"type":"user_prompt","session":1,"text":"bring the colony contract to metal","images":[]}
state: fold ok — sessions=1 tools=2 approvals=0
{"type":"goal_state_changed",...,"state":"acting","step":1,"max_steps":8,...}
{"type":"goal_state_changed",...,"state":"acting","step":2,...}
{"type":"goal_state_changed",...,"state":"acting","step":3,...}
{"type":"goal_state_changed",...,"state":"done","step":3,...}
loop: goal reached Done in 3 polls
APEXOS-RV: goal done — halting          # QEMU exits 0 via the sifive_test device
```

## Layout

| Path | What |
|---|---|
| `kernel/` | The `no_std` kernel: UART, heap, CLINT time, QEMU exit device, agent loop glue |
| `agent-core/` | Pure logic: SYNC-COPY of upstream `SystemState` + the goal driver (host-tested) |
| `vendor/apexos-protocol/` | Upstream wire contract @ pin `676aa38`, `no_std`-gated (`UPSTREAM.md` = provenance + patch ledger, the future upstream PR) |
| `xtest/` | The crown jewel: metal's captured UART stream parsed by the **pristine upstream** crate via a git dependency at the pin |
| `scripts/run-qemu.sh` | Timeout + marker grep + exit-code propagation |

## Upstream relationship

ApexOS-RS is never modified. The two shared pieces are vendored at a pinned commit with full provenance; the `no_std` patch is a minimal reviewable diff kept upstreamable; and every release must prove that the *unmodified* upstream crate deserializes this kernel's output. Hostile-input behavior rides along too: upstream's red-team suite (no adversarial frame may panic the decoder) runs against the vendored crate under both feature gates.

Apache-2.0. Vendored portions © upstream ApexOS-RS (same author).
