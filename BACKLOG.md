# BACKLOG — post-v1

Ideas parked deliberately so v1 could say no (PRD §5/§14). Promotion into PLAN phases happens explicitly, never by drift.

## Mesh & inference
- virtio-net + `smoltcp`; a2a mesh join; live inference served by a colony GPU/NPU node (Jetson/Orin-formfactor RISC-V NPU boards — Banana-Pi 60-TOPS class — are exactly this shape)
- Wire-path hardening on ingest: the vendored redteam suite already pins "no frame can panic the decoder"; keep it load-bearing when frames arrive from the network instead of a script

## Hardware
- Real-board bring-up on the incoming RISC-V hardware (model TBD when it lands) — the UART divisor-init breadcrumb in `uart.rs` and `cargo-binutils`/`objcopy -O binary` are the entry points
- RVA23 / vector exploration once target silicon supports it

## Kernel
- Multi-hart: real `critical-section` impl, per-hart stacks (`_max_hart_id`/`_hart_stack_size`), IPI wakeups (revisit PRD D4 + D5 together)
- Timer interrupts (CLINT) with a real trap handler → preemptive scheduling, or `embassy-executor` (P7.5 stretch — no concrete need appeared in v1)
- S-mode / OpenSBI boot path (PRD D2 alternative)
- `no_std` subset of `apexos-confine` semantics if metal ever grows storage

## Agent parity
- Operator paths on metal: `goal_resume` (detail `"resumed"`), `goal_cancel` (`"cancelled"`), approval gate (`"awaiting approval — {tool}"`)
- Persona port (upstream `persona.rs`, 104 LOC)
- Multi-goal scheduling; a `goals.json`-equivalent persistence story

## Upstream PRs to ApexOS-RS
- The `no_std` feature gate + `Ord` derives on ID newtypes — `vendor/apexos-protocol/UPSTREAM.md` ledger is the diff; the `xtest` cross-repo run is the proof
- `apexos-state` crate extraction so `agent-core/src/state.rs` stops being a SYNC-COPY
- `docs/repo-map.md` pointer from ApexOS-RS to this repo
