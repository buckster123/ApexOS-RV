# PRD — ApexOS-RV: a bare-metal RISC-V node class for the ApexOS colony

| | |
|---|---|
| **Status** | Draft v2 — reviewed in-repo against upstream source |
| **Owner** | buckster123 (André) |
| **Date** | 2026-07-19 |
| **Repo** | https://github.com/buckster123/ApexOS-RV *(this repo — standalone sibling of ApexOS-RS)* |
| **Upstream** | https://github.com/buckster123/ApexOS-RS @ `676aa3870ad7` (main, 2026-07-15) |
| **Companions** | `PLAN.md` (executable plan + status), `CLAUDE.md` (agent rules) |
| **Origin** | v1: reframed from a Gemini autonomous-agent prompt into a PRD + phased plan (as `metal/` inside ApexOS-RS). v2: verified line-by-line against upstream `676aa38`, corrected (see §2.4), and re-homed as its own repository |

---

## 1. Summary

ApexOS-RS today is a colony of Linux processes: every node — Pi Zero to DGX — runs `agentd` and friends on top of a full OS. **ApexOS-RV adds the first node class where the agent runtime *is* the firmware**: a `#![no_std]` Rust kernel for 64-bit RISC-V (`riscv64gc-unknown-none-elf`) that boots on QEMU's `virt` machine in milliseconds, speaks the colony's existing wire contract (`apexos-protocol`), and runs the goal-driver semantics natively on bare metal — no Linux, no libc, no runtime underneath.

It lives in its own repository. Upstream ApexOS-RS is never modified by this project; the two shared pieces (`apexos-protocol`, `state.rs`) are **vendored at a pinned commit** with full provenance, and the wire contract is proven compatible by deserializing this kernel's output with the *unmodified upstream* crate (§4 G7).

v1 is deliberately small: boot, serial diagnostics, heap, the ported protocol + state fold + goal-driver semantics, and a deterministic agent loop against a *scripted* inference stub, all verified headlessly in QEMU. Networking (and therefore live inference via the mesh) is explicitly post-v1.

## 2. Background

### 2.1 What exists upstream (portability audit, verified 2026-07-19 @ `676aa38`)

| Crate / module | Role | `std` surface | Bare-metal verdict |
|---|---|---|---|
| `apexos-protocol` (538 LOC) | Wire contract: `Event`, `ToolCall`, `ContentBlock`, `GoalState`, IDs… | `std::collections::HashMap` (exactly **one** field: `RegisterMcpServer.env`) + `std::fmt`; deps are serde + serde_json only (both support `no_std + alloc`) | **Vendor + feature-gate** — the keystone |
| `agentd/crates/core/src/state.rs` (350 LOC) | `SystemState` — a **pure event-fold** (sessions/tools/plugins/pending-approvals maps; `apply(&Event)`, no I/O, no time, no async) | Only `HashMap` + protocol types | **Vendor (SYNC-COPY)** — trivially portable |
| `agentd/crates/agentd/src/goal.rs` (728 LOC) | The **actual goal driver**: step-bounded Acting loop, `goal_step` verdicts, stall timeout, "LLM-proposes / code-disposes" | tokio (`Mutex`, mpsc, broadcast), `Arc`, `Instant`, `ToolProxy` | **Port the semantics, not the code** — fresh `no_std` implementation (§3) |
| `agentd/crates/core/src/persona.rs` (104 LOC) | Persona registry | `HashMap`, `Arc`, `Mutex` | Portable with light work — **post-v1** |
| `apexos-core` (rest), `apexos-agent`, `gateway`, `plugins`, `store`, `agentd` | Daemon, HTTP/WS, tools | tokio, reqwest, axum, image, fs | Out of scope on metal |
| `cerebro/*` | Cognitive memory | rusqlite, fastembed/ONNX | Impossible on bare metal; stays a **remote colony service** |
| `ui-slint`, `apex-tts`, `apex-stt`, `apexos-confine` | UI, voice, FS sandbox | GPU/ML/filesystem | Out of scope |

No RISC-V or `no_std` work exists upstream; this repo is greenfield.

### 2.2 Why

- **Thesis completion.** "Agent-first OS" currently means "agent-first Linux distro." A node whose boot vector *is* the agent loop is the strongest possible version of the claim — and a great story for both READMEs.
- **RISC-V is the right ISA for it**: open, QEMU-first-class, RVA23 now ratified, with cheap real silicon (VisionFive 2, Milk-V class — real hardware is already inbound here) when v1 outgrows simulation.
- **The colony architecture already solves the hard part.** ApexOS-RS nodes federate and let a GPU node serve inference to the cluster. A compute-poor bare-metal node is only meaningful *because* that mesh exists — it can eventually join as a client instead of pretending to run models locally.

### 2.3 What changed from the Gemini draft (v1)

The original document was an autonomous system prompt ("FULL AUTO-MODE, do not ask permission"). v1 kept its sound technical spine (riscv-rt, QEMU `virt`, NS16550A at `0x1000_0000`, memory.x, allocator, phased gates) and fixed the gaps: named the real portable crates, started at `-smp 1`, added a machine-checkable pass/fail exit (SiFive test device), confronted the fact that bare metal has **no network and therefore no live LLM** in v1, and isolated all work so the host build cannot break.

### 2.4 What changed in v2 (this document)

Verified against upstream source at `676aa38`; four corrections:

1. **`state.rs` is not the goal state machine.** It is `SystemState`, a pure event-fold — still ported (it's the canonical state semantics, and trivially `no_std`-able), but the goal lifecycle lives in `agentd/src/goal.rs` (728 LOC of tokio). The metal agent loop is therefore a **fresh `no_std` implementation of `goal.rs` semantics** (verdict enum `Continue/Done/Blocked`, 1-indexed in-flight step, `max_steps` budget → `Done`, stall → `Failed`), not a port of its code.
2. **Upstream never emits `Planning`/`Reflecting`.** Those `GoalState` variants are reserved-but-unused ("P2a uses Acting / Done / Failed; the rest are reserved"). v1's scripted walk (`Planning → Acting → Reflecting → Done`) would have exercised states the colony never produces. The metal run now mirrors the real lifecycle: `Acting(1) → … → Acting(n) → Done` (plus `Blocked`/`Failed`/`Cancelled` paths), emitting `GoalStateChanged` with upstream's exact field set.
3. **Standalone repo, not `metal/`.** The root-workspace guardrail is replaced by a vendoring/provenance policy (§8); the cross-world wire test gets *stronger* — metal output parsed by the pristine upstream crate pulled straight from GitHub (§4 G7).
4. **Version placeholders were stale** (they were labeled as such): current pins as of 2026-07-19 are `riscv-rt 0.18.0`, `riscv 0.16.1`, `embedded-alloc 0.7.0`. Still resolved for real at P1.3.

## 3. Product framing — what a metal node *is*

Not a smaller `agentd`. A **new node class**:

> A deterministic, instant-boot agent substrate. It holds goals, folds events through the colony's own `SystemState`, drives the goal lifecycle with upstream's exact semantics ("LLM-proposes / code-disposes"), emits typed `apexos-protocol` events — and treats inference as an external service, exactly the way the colony already treats a GPU node.

In v1 the "external service" is a compiled-in script (deterministic, testable). Post-v1 it becomes the real mesh over virtio-net. Cerebro memory, voice, face, and self-update never move to metal; those remain the Linux node's job.

## 4. Goals (v1)

| # | Goal | Acceptance |
|---|---|---|
| G1 | Kernel builds for `riscv64gc-unknown-none-elf` on stable Rust | `cargo build` succeeds |
| G2 | Boots on QEMU `virt` (`-bios none`, M-mode, `-smp 1`) and prints a banner over the NS16550A UART | Banner visible via `cargo run` |
| G3 | `println!`-style logging + panic handler that reports the panic and exits QEMU with a failure code | Forced panic produces message + nonzero exit |
| G4 | Heap allocator over a static region; `alloc` types (`Vec`, `String`, `format!`) work | Allocation smoke test passes in-kernel |
| G5 | Vendored `apexos-protocol` compiles `no_std + alloc` behind a feature gate, **without changing the JSON wire format**, and round-trips events in-kernel — including the float-bearing and `serde_json::Value`-bearing variants | In-kernel serialize→deserialize test passes; vendored crate's own wire-lock tests still green under default (std) features |
| G6 | Goal-driver semantics (per `goal.rs`) run on metal, driven by a scripted `Inference` impl: verdicts walk `Acting(1) → … → Done`; budget exhaustion → `Done`; a stall path → `Failed`; every transition emits `GoalStateChanged` (upstream's exact fields) as one JSON object per line over UART. The vendored `SystemState` folds a scripted event sequence on metal with asserted post-conditions | Scripted run reaches `Done`; fail-scripted build reaches `Failed` + nonzero exit; fold assertions pass |
| G7 | Headless verification: one script runs QEMU with a timeout, checks output markers, and returns QEMU's exit code; **captured UART JSON deserializes with the pristine upstream `apexos-protocol` (git dependency on ApexOS-RS @ pin, `std`, unmodified)** | `scripts/run-qemu.sh` exits 0; host cross-check test passes |

G7's cross-check is the point of the whole exercise: the colony's own "load-bearing safety boundary" — typed protocol deserialization — proven across both the std/no_std divide *and* the repo boundary.

## 5. Non-goals (v1)

No networking (⇒ no live LLM calls, no mesh join), no MMU / S-mode / user-space isolation, no real hardware bring-up (QEMU only — the inbound board waits for post-v1), no Cerebro port, no UI/voice/sensors, no self-update loop, no multi-hart scheduling (secondary harts stay parked), no async executor (a cooperative tick loop is enough for v1). And the prime directive, restated for the standalone world: **upstream ApexOS-RS is read-only; vendored code changes only through the provenance discipline in §8, and the wire format never changes at all.**

## 6. Users & scenarios

- **Primary (now):** André + Claude Code executing `PLAN.md` phase by phase; each phase gate is independently verifiable so agent sessions can be scoped and audited.
- **Secondary (post-v1):** a colony operator flashes a $10–$60 RISC-V board that cold-boots into a mesh-attached agent in under a second; a researcher uses ApexOS-RV as a minimal deterministic harness for agent-loop experiments.

## 7. Functional requirements

| ID | Requirement |
|---|---|
| FR-1 | This repo is a single Cargo workspace: `kernel/` (the `no_std` bin), `agent-core/` (pure logic, host-testable), `vendor/apexos-protocol/` (vendored crate), `xtest/` (host-side cross-checks), `scripts/` |
| FR-2 | Boot via `riscv-rt` `#[entry]`; link layout from a project `memory.x` (RAM origin `0x8000_0000`) |
| FR-3 | NS16550A driver at `0x1000_0000` implementing `core::fmt::Write`; `print!`/`println!` macros |
| FR-4 | `#[panic_handler]` prints payload + location, then exits QEMU via the SiFive test device (`0x10_0000`) with a failure code |
| FR-5 | Global allocator (embedded-alloc) over a static heap region; OOM policy = panic |
| FR-6 | Vendored `apexos-protocol` gains `std` (default) / `alloc` features; the one `HashMap` field goes through a `Map` alias (`HashMap` under std, `BTreeMap` under `no_std`) — identical JSON representation. The whole patch is one reviewable diff against the pristine copy, written to be upstreamable to ApexOS-RS |
| FR-7 | `state.rs` vendored as a SYNC-COPY (header records upstream path + commit) into `agent-core`, with the same `Map` treatment; its unit tests keep running on host |
| FR-8 | `agent-core` implements the goal driver as pure `no_std` logic mirroring `goal.rs` semantics: `Verdict { Continue(Option<steer>), Done, Blocked(reason) }`, 1-indexed in-flight `step`, `max_steps` budget (exhaustion → `Done`, per upstream), stall detection → `Failed`, `detail` strings matching upstream's. An `Inference` trait supplies verdicts; v1 ships `ScriptedInference` (compiled-in deterministic transcript) |
| FR-9 | `scripts/run-qemu.sh`: timeout, marker grep, exit-code propagation; usable locally and in CI |
| FR-10 | `xtest` deserializes a captured UART log with the **upstream** `apexos-protocol` via a git dependency pinned to the audited commit — no local code in the loop |

## 8. Non-functional requirements

- **Provenance (replaces the old root-workspace guardrail):** `vendor/` and SYNC-COPY files change only in dedicated commits; `vendor/apexos-protocol/UPSTREAM.md` records the source repo, pinned commit, file list, and every local patch. Step one of vendoring is a **pristine, byte-identical copy** (its own commit); the `no_std` gate is a separate reviewable diff on top — that diff is the future upstream PR.
- **Unsafe policy:** `unsafe` confined to HAL modules (`uart`, `heap`, `qemu`, boot glue), every block with a `// SAFETY:` comment.
- **Determinism:** given the same scripted transcript, byte-identical UART event log across runs.
- **Toolchain:** stable Rust (1.97.0 recorded at P0) + `riscv64gc-unknown-none-elf` target (auto-installed via `rust-toolchain.toml`); host deps via apt (`qemu-system-misc`, `gdb-multiarch`).
- **Docs:** `CLAUDE.md` governs agent work here; README/BACKLOG live in this repo; Apache-2.0 attribution preserved on everything vendored (upstream copyright line + license text ride along in `vendor/`).

## 9. Design decisions (v1)

| # | Decision | Rationale / alternative |
|---|---|---|
| D1 | **Standalone repo; upstream code vendored at a pin** (pristine commit + patch commit), never submoduled | Zero coupling to upstream's build; provenance stays auditable; the no_std patch doubles as an upstream PR. Alt (rejected): git submodule (drags the whole repo in), git-dep-only (can't apply the feature gate) |
| D2 | `-bios none`, kernel at `0x8000_0000`, machine mode | Simplest ownership of the machine; riscv-rt supports M-mode directly. Alt (documented for later): OpenSBI + S-mode at `0x8020_0000` |
| D3 | `riscv-rt` for boot/runtime | Standard, maintained; hand-rolled asm boot is a non-goal |
| D4 | `-smp 1` until the loop is proven | Multi-hart adds locking + stack-sizing complexity for zero v1 value |
| D5 | `embedded-alloc` + the `riscv` crate's single-hart critical-section impl | Smallest working `#[global_allocator]`; the single-hart CS impl is valid exactly because of D4 (revisit both together) |
| D6 | `BTreeMap` (not hashbrown) for the `no_std` protocol map | In `alloc` itself → no new dependency; identical JSON objects; deterministic ordering is a bonus. Requires `K: Ord` — the single affected field is `HashMap<String, String>`, so yes |
| D7 | Cooperative tick loop, not async | `goal.rs`'s control plane is deterministic code around an async turn engine; on metal the turn engine is scripted, so a tick loop reproduces the semantics without an executor. `embassy-executor` is the sanctioned escape hatch if a real need appears |
| D8 | SiFive test device for exit codes | Turns every phase gate into `$?` — scriptable, CI-ready, agent-verifiable |
| D9 | Workspace defaults tuned for the kernel: `.cargo/config.toml` sets `build.target = riscv64gc-unknown-none-elf` with target-scoped rustflags/runner, and `default-members = ["kernel"]`; host-side testing goes through the `cargo hosttest` / `cargo hostcheck` aliases (explicit host triple) | One config at the repo root that applies from anywhere — kills the config-discovery trap (a `kernel/.cargo/config.toml` would be **silently ignored** when running cargo from the repo root, since cargo reads config from the CWD upward, not from the package dir). Bare `cargo build`/`run` do the obvious thing; the one documented trap is that bare `cargo test` cross-compiles (use `cargo hosttest`) |
| D10 | Naming: crates `apexos-rv-*`; UART markers `apexos-rv: hart 0 online` / `APEXOS-RV: goal done — halting` | Matches the repo identity; markers are contracts shared with `scripts/run-qemu.sh` |

## 10. Milestones

| M | Name | Exit criterion | PLAN phase |
|---|---|---|---|
| M0 | Host bootstrap | Toolchain + QEMU verified; upstream pinned | 0 |
| M1 | Workspace scaffold | Repo builds an empty kernel for the target | 1 |
| M2 | First boot | Banner over UART via `cargo run` | 2 |
| M3 | Diagnostics | `println!`, printing panic handler, QEMU exit device | 3 |
| M4 | Heap | alloc smoke test green | 4 |
| M5 | Protocol on metal | G5 acceptance | 5 |
| M6 | Agent core | G6 acceptance | 6 |
| M7 | Agent loop + harness | G7 acceptance; docs updated | 7–8 |

## 11. Risks

| Risk | Mitigation |
|---|---|
| Crate API drift (riscv-rt 0.18 / riscv 0.16 / embedded-alloc 0.7 move fast; 0.18's boot/trap surface differs from older tutorials) | Pin exact versions in Phase 1; PLAN Appendix A snippets are explicitly "verify against the pinned version's docs" |
| Vendored code drifts from upstream | `UPSTREAM.md` pins the commit; `xtest` parses metal output with upstream-at-pin; a periodic (post-v1) sync check bumps the pin deliberately, never silently |
| `serde_json` no_std edge cases | The risky surfaces are **confirmed present**: `f32` (`SensorReading`, `CouncilRoundDone.convergence`), `f64` (`VastInstanceLaunched.cost_per_hr`), `serde_json::Value` (`ToolCall.args`, `ContentBlock`). G5's in-kernel round-trip exercises exactly these variants |
| Goal-semantics fidelity (fresh implementation, not a port) | FR-8 enumerates the load-bearing behaviors from `goal.rs` (verdicts, 1-indexed step, budget→`Done`, stall→`Failed`, detail strings); host unit tests in `agent-core` assert each; the emitted event shape is locked by G7 |
| Silent boot failures (nothing on serial) | Phase 2 gotcha list + GDB crib in PLAN Appendix C (`-s -S`, `readelf -h`, `-d in_asm`) |
| Scope creep toward networking/real boards | This section. Post-v1 list exists precisely so v1 can say no |

## 12. Open questions

1. Board target for post-v1 hardware bring-up — **hardware is already on the way here**; pin down the exact model when it lands (affects nothing in v1, QEMU `virt` only).
2. `ScriptedInference` transcripts as Rust consts or embedded JSON assets? (Lean: consts for v1.)
3. When to open the upstream PR offering the `no_std` feature gate to ApexOS-RS: after M5 (patch proven in-kernel) or after M7 (full story with the cross-repo test as evidence)? (Lean: after M7 — the xtest result *is* the PR's proof.) — **Resolved at v1 (2026-07-19): after M7, as leaned; the patch grew a second improvement en route (`Ord` on ID newtypes) and the green xtest run is the PR's evidence. Filed in BACKLOG §Upstream.**

## 13. Definition of done (v1)

`git clone https://github.com/buckster123/ApexOS-RV && cargo run --release` on a fresh Ubuntu box with the Phase-0 prerequisites boots the kernel in QEMU, streams a deterministic sequence of protocol events over serial while a scripted goal walks `Acting 1/n → … → Done`, prints `APEXOS-RV: goal done — halting`, and exits 0 — and `cargo hosttest` deserializes that captured stream with the **unmodified upstream** `apexos-protocol` (git dependency @ pin). Byte-identical UART logs across two consecutive runs. Docs: PLAN checklists complete, README + BACKLOG updated.

## 14. v2 — the mesh arc (charter, 2026-07-19)

v1 proved the substrate; v2 completes the thesis: **the metal node consumes the colony.** Networking arrives and the scripted inference stub is joined by a live one — a goal on bare metal driven by a real colony LLM over the existing wire contract.

### v2 goals

| # | Goal | Acceptance |
|---|---|---|
| G8 | virtio-net NIC up on QEMU virt (virtio-mmio, polled) with a deterministic MAC | `net: virtio-net up …` banner; v1 flow unaffected |
| G9 | smoltcp TCP/IP woven into the cooperative loop (static slirp config: `10.0.2.15/24` → gw `10.0.2.2`; `mtime` → smoltcp clock) | TCP round-trip against a host-side mock, gated by exit code |
| G10 | WebSocket client speaks the gateway contract: handshake (+ optional bearer), consume `session_init`, exchange raw `Event` frames | Mocked-gateway session established, deterministic gate |
| G11 | `MeshInference`: each goal step = one gateway turn (`user_prompt` out → `agent_text` deltas → `turn_complete` → verdict); the P7 mtime watchdog stalls quiet turns — **now over a real socket**. Scripted inference remains the CI default | Mocked 3-step goal ends `Done`, byte-identical two-run gate, exit 0; silent-mock variant → `Failed`/exit 2; live-colony demo script documented (not CI-gated) |

### v2 non-goals

No mDNS/peer discovery (configured gateway address only), no TLS (LAN plaintext WS + bearer, matching current colony practice), no serving of anything (pure client), no upstream ApexOS-RS changes (the documented `/ws` contract as-is), no interrupts (smoltcp is poll-native; the D7 loop absorbs it).

### v2 design decisions

| # | Decision | Rationale |
|---|---|---|
| D11 | `virtio-drivers` + `smoltcp` + `embedded-websocket` (all `no_std`) | Battle-tested rust-osdev NIC driver over virtio-mmio; poll-native stack fits the cooperative loop; RFC6455 without an executor. Versions pinned at P9.1 from crate source, per rule 6 |
| D12 | QEMU user-mode networking (`-netdev user` + `-device virtio-net-device`, explicit `mac=`) | No sudo/tap; guest dials out; `10.0.2.2` reaches host services (the mock) and the LAN gateway reaches the real colony. Explicit MAC keeps logs deterministic |
| D13 | Gates run against a **repo-local mock gateway** (`xtest` host bin speaking the documented contract); the live-colony run is a demo script, never a gate | Deterministic CI with real network I/O; LLM text is inherently nondeterministic so it proves markers, not bytes |
| D14 | Verdicts over the wire v2-style: the step directive instructs the model to end with a `GOAL_STEP: continue|done|blocked …` line; a missing verdict is `Continue(None)` — upstream's own default. The daemon-side `goal_step` tool isn't reachable from a frontend session | Keeps driver semantics untouched; pragmatic, documented, replaceable post-v2 if upstream grows a remote-turn RPC |

### v2 definition of done

`cargo hosttest` green including mock-gateway tests; the mocked mesh run walks a 3-step goal to `Done` deterministically (byte-identical two-run diff) and the silent-mock run fails by *timeout* with upstream's stall detail; `scripts/run-live.sh` (gateway URL + token via env) documented with a captured live-colony transcript checked into `docs/` as evidence, marked non-normative.

## 15. Future work (explicitly post-v2)

Full a2a mesh membership beyond the client role (mDNS discovery, peers.toml presence, sensor-bridge feed); multi-hart with a real `critical-section` impl; timer interrupts (CLINT) + preemptive scheduling or embassy; S-mode/OpenSBI path; persona port; **real-board bring-up on the inbound hardware** (likely JH7110/VisionFive-2 or Milk-V class — RVA23-profile silicon is arriving industry-wide); upstream PRs to ApexOS-RS (the `no_std` protocol gate, an `apexos-state` crate extraction so `state.rs` stops being a copy, a `docs/repo-map.md` pointer to this repo); `no_std` subset of `apexos-confine` semantics if metal ever gets storage.
