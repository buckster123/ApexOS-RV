# BACKLOG — post-v1

Ideas parked deliberately so v1 could say no (PRD §5/§14). Promotion into PLAN phases happens explicitly, never by drift.

## Mesh & inference
- virtio-net + `smoltcp`; a2a mesh join; live inference served by a colony GPU/NPU node (Jetson/Orin-formfactor RISC-V NPU boards — Banana-Pi 60-TOPS class — are exactly this shape)
- Wire-path hardening on ingest: the vendored redteam suite already pins "no frame can panic the decoder"; keep it load-bearing when frames arrive from the network instead of a script

## Hardware
- **First boards chosen (2026-07-19, André + APEX via occipital): Milk-V or StarFive class** — exact model IDs pending market availability. Entry points: the UART divisor-init breadcrumb in `uart.rs`, `cargo-binutils`/`objcopy -O binary`, a per-board `memory.x`
- **The minimum-viable-substrate challenge:** how small a RISC-V device can run the agent loop? Vanilla Linux floored colony nodes at Pi-Zero-2W/512 MB; this is firmware — a static image with a 1 MiB default heap. Waypoints: measure the real RAM floor on rv64 (shrink heap + net buffers), then rv32 (`riscv32imc`-class — ESP32-C3/CH32V territory; protocol and agent-core are alloc-clean, the HAL is the port), then find the smallest thing that can hold a goal state machine
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
- ~~The `no_std` feature gate + `Ord` derives on ID newtypes~~ — **MERGED upstream 2026-07-19** (landed by the ApexOS-RS session; features block confirmed on `main` @ `f4b67b4`). Follow-up here: **bump the pin past the merge, re-vendor pristine (the local patch ledger empties), re-run xtest at the new pin**
- `apexos-state` crate extraction so `agent-core/src/state.rs` stops being a SYNC-COPY
- `docs/repo-map.md` pointer from ApexOS-RS to this repo
