# CLAUDE.md — ApexOS-RV (bare-metal RISC-V node class for the ApexOS colony)

> Project intent lives in `PRD.md`; the executable plan and current status live in `PLAN.md` (its checklists are the source of truth — keep them updated). Upstream ApexOS-RS is a **separate, read-only** repository; everything shared is vendored here at a pinned commit (see PLAN header).

## What this is

A standalone Cargo workspace building `apexos-rv-kernel`: a `#![no_std]` `#![no_main]` kernel for `riscv64gc-unknown-none-elf`, booted by `riscv-rt` on QEMU's `virt` machine (`-bios none`, machine mode, `-smp 1`). It runs the ApexOS colony's wire contract (vendored `apexos-protocol`), its `SystemState` event-fold (SYNC-COPY of upstream `state.rs`), and a fresh `no_std` implementation of the goal-driver *semantics* from upstream `goal.rs`, driven by scripted inference in v1. It is intentionally **not** a port of `agentd`/`cerebro`/UI.

## Commands

```bash
# all from the repo root — .cargo/config.toml here makes the RISC-V target the default
cargo build                    # cross-compiles the kernel (default-members = ["kernel"])
cargo run                      # boots QEMU (runner); Ctrl-A X to exit
cargo hosttest                 # host-side tests: agent-core, vendored protocol, xtest  [alias]
cargo hostcheck                # host-side cargo check of the same                      [alias]
./scripts/run-qemu.sh target/riscv64gc-unknown-none-elf/debug/apexos-rv-kernel   # gated, scripted run
# debug: add -s -S to QEMU, then: gdb-multiarch <elf> -ex 'target remote :1234'
```

**Trap:** bare `cargo test` cross-compiles tests for RISC-V and fails — always `cargo hosttest`. The host triple is hardcoded in the aliases (this box: x86_64-unknown-linux-gnu).

## Hard rules

1. **Vendored code is provenance-tracked.** `vendor/**` and any `// SYNC-COPY` file changes only in dedicated commits; `vendor/apexos-protocol/UPSTREAM.md` records the pin SHA and every local patch. Vendoring is two commits: pristine byte-identical copy, then the reviewable patch (that diff is the future upstream PR). Never mix vendored-code edits into feature commits; never bump the pin silently (dated PLAN Changelog entry).
2. **Wire format is load-bearing** (upstream AGENTS.md doctrine: typed `apexos-protocol` deserialization is a safety boundary). Never change what protocol types serialize to. Representation swaps must be proven equivalent (`HashMap`→`BTreeMap`: same JSON object, order-insensitive assertion). The `xtest` cross-repo check (pristine upstream crate parsing our UART output) is the final arbiter; if a change would alter bytes-on-the-wire semantics, stop and surface it.
3. **`unsafe` stays in the HAL.** Only `uart.rs`, `qemu.rs`, `heap.rs`, `time.rs`, and boot/trap glue may contain `unsafe`, and every block carries a `// SAFETY:` comment naming the invariant. `agent-core` is 100% safe, `no_std`, and host-testable.
4. **Gates are hard.** A phase's acceptance block in PLAN must pass — shown as commands + exit codes — before its boxes are checked or the next phase starts. Never weaken a gate to pass it; write a dated note in PLAN's Changelog instead and stop.
5. **No scope drift.** No networking, interrupts (beyond riscv-rt defaults), MMU/S-mode, multi-hart execution, async executors, or real-hardware code in v1 unless PLAN explicitly says so. New ideas go to BACKLOG.md, not into the tree.
6. **Version drift is recorded, not fought silently.** `riscv-rt`/`riscv`/`embedded-alloc` APIs move fast (riscv-rt 0.18 ≠ the 0.12-era tutorials). Pin exact versions (P1.3); when reality differs from a PLAN snippet, follow the pinned crate's docs and log one line in PLAN's Changelog.
7. **Goal semantics mirror upstream, not intuition.** The driver's behaviors come from `goal.rs` @ the pin: verdicts `Continue/Done/Blocked`, 1-indexed in-flight `step`, budget exhaustion → `Done`, stall → `Failed`, upstream's `detail` strings — and `Planning`/`Reflecting` are never emitted (reserved upstream). Divergence is a bug or a surfaced decision, never an improvisation.
8. **Determinism.** Same build + same script ⇒ byte-identical UART log. No timestamps, addresses, or randomness in output. (The Phase 7 gate diffs two runs.)
9. **Attribution.** Apache-2.0: keep notices, state changes — upstream's copyright rides along with everything vendored (crate-local LICENSE + SYNC-COPY headers).

## Conventions

- Stable Rust only; edition 2021 to match upstream. `-smp 1` everywhere until Phase 7 exit says otherwise.
- Commits: small, one PLAN task each, message prefixed with the task id (e.g. `P3.4: panic handler reports over UART and exits QEMU fail`).
- Output markers are contracts: `apexos-rv: hart 0 online`, `KERNEL PANIC:`, one JSON event per line, `APEXOS-RV: goal done — halting`. Scripts grep them — change marker text only together with `scripts/run-qemu.sh` and PLAN.
- Kernel owns I/O; `agent-core` owns logic. If a change makes `agent-core` need MMIO or `alloc`-beyond-collections, the boundary is being violated — redesign instead.

## Memory & continuity (cerebro-cortex)

The cerebro-cortex MCP is this project's long-term memory; the built-in memory files stay lean (index-level) while cerebro grows into the knowledgebase. Do these unprompted:

- **Phase-gate checkpoint:** when a phase's acceptance passes, `memory_store` a checkpoint (tags `apexos-rv`, `checkpoint`, `phase-N`) with gate evidence + deviations.
- **Session save:** before a session ends or context rolls over — and before risky operations — `session_save` with phase state, in-flight work, next action. After a crash/OOM: `session_recall` + `recall` (tag `apexos-rv`) to rebuild, then PLAN.md checklists are ground truth on any conflict.
- **Intentions:** for ideas André voices mid-session or explicitly parks — not a phase-todo mechanism (PLAN.md checklists are the todo list; the forward pointer lives in the latest checkpoint).
- Insights worth keeping (API-drift discoveries, debugging war stories, upstream findings) → regular memories tagged `apexos-rv`.

## When stuck

Reproduce minimally, read the pinned crate's docs for the exact version (not memory of older APIs), use the Appendix C debug crib (GDB halt, `readelf`, trap-cause print). For upstream-behavior questions, read the source at the pin (`goal.rs`, `state.rs`, protocol tests) — never guess. If a session ends mid-phase: leave the tree building, boxes accurate, and a dated Note in PLAN describing exact state and next action.
