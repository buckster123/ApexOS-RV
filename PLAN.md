# PLAN — ApexOS-RV implementation plan

Executable companion to `PRD.md`. Task IDs (P2.3 = Phase 2, task 3) are for commit messages and session notes. Check boxes off as work lands; the checklist state in this file is the project's source of truth.

Upstream reference: **ApexOS-RS @ `676aa3870ad7e2b469be1dcaec23498c943491a9`** (main, 2026-07-15) — "the pin". All vendored code and the xtest git dependency use this commit until the pin is bumped deliberately (dated Changelog entry).

## 0. How to drive this plan with Claude Code

1. **Session scope:** one phase (two small ones at most) per session. First message: paste the kickoff prompt from Appendix B, adjusted for the phase.
2. **Plan first, then execute.** Start in plan mode (`Shift+Tab` to cycle to it, or launch with `claude --permission-mode plan`). When the proposed plan matches this file's phase, approve it — auto mode fits the mechanical build→error→fix loops this project is full of. Use `--dangerously-skip-permissions` only inside a disposable VM/container.
3. **Gates are hard.** Do not start phase N+1 until phase N's acceptance block passes and the boxes are checked. If a gate can't be met, write a dated note under the phase and stop — don't quietly weaken the gate.
4. `CLAUDE.md` is auto-loaded context; it carries the hard rules (vendoring/provenance, unsafe policy, wire-format invariant).

---

## Phase 0 — Host bootstrap & upstream pin

**Objective:** toolchain ready; upstream commit pinned.

- [x] P0.1 `rustup target add riscv64gc-unknown-none-elf` (stable toolchain; version recorded below) — *done 2026-07-19*
- [x] P0.2 `sudo apt-get install -y qemu-system-riscv gdb-multiarch build-essential` — **note:** on Ubuntu 26.04 / QEMU 10.x, riscv64 moved out of `qemu-system-misc` into its own `qemu-system-riscv` package (older guides say `-misc`) — *done 2026-07-19*
- [x] P0.3 `qemu-system-riscv64 --version` ≥ 7.x — *QEMU 10.2.1 ✓ 2026-07-19; Phase 0 gate closed*
- [x] P0.4 Pin upstream: record the ApexOS-RS main SHA this project audits/vendors against (header above). For vendoring steps, obtain files at the pin via a fresh shallow clone or `git -C ~/Projects/ApexOS-RS worktree`/`git show` at that SHA — never from an unpinned working tree that may have drifted. — *done 2026-07-19*

**Acceptance:** all four boxes checked; versions note written below.

**Notes:**
- 2026-07-19: rustc 1.97.0 (2d8144b78 2026-07-07), cargo 1.97.0. Target riscv64gc-unknown-none-elf installed. Upstream pinned at `676aa38` (see header). Current crate versions on crates.io: riscv-rt 0.18.0, riscv 0.16.1, embedded-alloc 0.7.0, critical-section 1.2.0 (P1.3 re-verifies at implementation time). QEMU + gdb-multiarch initially absent (P0.2/P0.3).
- 2026-07-19 (later): gdb-multiarch 17.1 ✓, build-essential ✓. QEMU 10.2.1 system packages landed but riscv64 lives in the (not-yet-installed) `qemu-system-riscv` split package — P0.2 command corrected above; P0.3 blocked on it.

---

## Phase 1 — Workspace scaffold

**Objective:** this repo cross-compiles an empty kernel.

- [x] P1.1 Root `Cargo.toml` per Appendix A: `[workspace] members = ["kernel"]`, `default-members = ["kernel"]`, `resolver = "2"`, plus `[workspace.dependencies]` mirroring upstream's `serde`/`serde_json` lines (so the pristine vendored crate builds unchanged in P5.1)
- [x] P1.2 Root `.cargo/config.toml`: `build.target`, target-scoped rustflags + QEMU runner, `hosttest`/`hostcheck` aliases — **deviation from Appendix A:** single `-Tlink.x` flag (see Changelog, riscv-rt 0.18 `memory` feature)
- [x] P1.3 Pinned from crate source (registry read, not tutorials): riscv-rt 0.18.0 (`memory` feature), riscv 0.16.1 (`critical-section-single-hart` confirmed present), embedded-alloc 0.7.0
- [x] P1.4 `kernel/` bin crate `apexos-rv-kernel` with `memory.x` (REGION_ALIAS scheme + `_heap_size` confirmed still current in 0.18's link.x.in) and `build.rs`
- [x] P1.5 Minimal `main.rs`: `#![no_std] #![no_main]`, `riscv_rt::entry` fn with `loop {}`, trivial panic handler (`loop {}` for now)

**Acceptance:** `cargo build` succeeds; `file target/riscv64gc-unknown-none-elf/debug/apexos-rv-kernel` says ELF 64-bit RISC-V; `cargo hostcheck` reports nothing to check yet without erroring on the kernel (excluded).

**Gotchas:** cargo reads `.cargo/config.toml` from the **current directory upward** — that's why it lives at the repo root, not in `kernel/` (a `kernel/.cargo/config.toml` is silently ignored when you run cargo from the root). `default-members = ["kernel"]` keeps bare `cargo build`/`run` kernel-only so host-side members added later don't get cross-compiled with wrong features.

---

## Phase 2 — First boot: banner over serial

**Objective:** proof of life. QEMU boots our ELF and bytes appear.

- [x] P2.1 Crude UART poke: raw `write_volatile` of a banner string to `0x1000_0000` (no driver yet), first line of the entry fn
- [x] P2.2 Confirm the cargo runner works: `cargo run` invokes `qemu-system-riscv64 -machine virt … -smp 1 -bios none -kernel <elf>`
- [x] P2.3 Confirm load layout: `readelf` entry point = `0x80000000`; all three LOAD segments inside RAM (RW memsz reaches region top — that's `.bss`+`.stack`, see Changelog DTB note)
- [x] P2.4 Banner prints; kernel then parks in `loop { wfi }` — *all verified 2026-07-19, log captured*

**Acceptance:** `cargo run` shows `apexos-rv: hart 0 online`; Ctrl-A X exits QEMU.

**Gotchas (silent-boot triage, in order):** wrong entry/load address (P2.3); rustflags not applied (run `cargo build -v` and look for the `-T` link args; config discovery is the usual culprit — see Phase 1); memory.x not found (build.rs link-search); link-arg order (memory.x **before** link.x). Appendix C has the GDB recipe.

---

## Phase 3 — Real UART driver, `println!`, panics that report, scripted exit

**Objective:** a diagnostics floor sturdy enough that every later phase is checkable by exit code + grep.

- [x] P3.1 `uart.rs`: NS16550A driver — poll LSR (offset 5) bit 5 (THR empty) before writing THR (offset 0); `impl core::fmt::Write`; breadcrumb for real-hw divisor init left
- [x] P3.2 `print!` / `println!` macros wrapping the writer in `critical_section::with` (impl = riscv 0.16 `critical-section-single-hart`; `critical-section = "1.2"` added as the API dep)
- [x] P3.3 `qemu.rs`: SiFive test device at `0x10_0000` — `0x5555` = exit(0); `((code as u32) << 16) | 0x3333` = exit(code)
- [x] P3.4 Panic handler: prints `KERNEL PANIC: {info}` then `qemu::exit_fail(1)`
- [x] P3.5 `#[cfg(feature = "panic-test")]` intentional-panic path — kept in-tree, **inert without the feature** (the "disable" reading; re-runnable negative test for CI) — *gate evidence 2026-07-19: normal run exit 0; panic-test run prints `KERNEL PANIC: panicked at kernel/src/main.rs:15:5` and exits 1*

**Acceptance:** normal run prints banner and exits 0 via `exit_pass()`; `cargo run --features panic-test` prints the panic message **and the shell sees a nonzero exit code**. Delete/disable the test feature after capture.

---

## Phase 4 — Heap & `alloc`

**Objective:** `Vec`/`String`/`format!` on metal.

- [x] P4.1 Static `[MaybeUninit<u8>; N]` region taken (version-proof route; linker `_heap_size` left unused)
- [x] P4.2 `heap.rs`: `embedded-alloc` `LlffHeap` as `#[global_allocator]` (slimmed to `default-features = false, features = ["llff"]` per D5), init once at boot; `unsafe` confined here with SAFETY invariant
- [x] P4.3 `extern crate alloc;` + smoke test: `format!` → `Vec` push → print → drop; deterministic output `alloc ok: 42 (heap: 1024 KiB)`
- [x] P4.4 OOM policy documented in `heap.rs`: alloc failure → default alloc-error path → panic → our reporting handler + `exit_fail(1)`. Heap = **1 MiB** default, **1 KiB** under the repeatable `tiny-heap` gate feature — *evidence 2026-07-19: normal exit 0; tiny-heap prints `memory allocation of 4096 bytes failed` and exits 1 immediately*

**Acceptance:** smoke test output visible; run exits 0; forced tiny-heap build (temporarily set heap to 1 KiB) panics *with a readable message* rather than hanging.

---

## Phase 5 — `apexos-protocol` on metal (vendor + upstream-quality gate)

**Objective:** the colony's wire contract compiles on metal with an **unchanged JSON representation**. Two-commit discipline: pristine copy, then the reviewable `no_std` patch (that diff is the future upstream PR).

- [x] P5.1 **Pristine vendor commit** `ae39d16`: byte-identical (`diff -r` clean), crate inventory turned out to be 4 files (Cargo.toml, src/lib.rs, tests/redteam.rs, README.md — the redteam adversarial suite rides along!); upstream LICENSE + UPSTREAM.md added. Gate: 6 wire-lock + 8 redteam tests green, untouched
- [x] P5.2 **Patch commit** `fc53348`: exactly as specified; lib.rs needed **no** test fix (the `std::collections::HashMap::new()` test lives in `state.rs` — P6 handles it there). UPSTREAM.md ledger updated
- [x] P5.3 Host proofs: hosttest 15/15 under std; alloc-only check green; **bonus: the full suite (wire-lock + redteam + new wire_compat) passes under `--no-default-features --features alloc` too — 15/15 on the BTreeMap side**
- [x] P5.4 Metal consumption: in-kernel round-trip of `SensorReading::AirQuality` (f32 batch), `VastInstanceLaunched` (non-dyadic f64 0.297), `ToolRequested` (Value args tree) — byte-stable and value-tree-stable, streamed line-delimited over UART. Note: kernel modules outside `uart`'s textual scope import the print macros by path (`use crate::println;`)
- [x] P5.5 Reviewer-grade messages on both commits; the `fc53348` diff vs `ae39d16` is the upstream PR artifact — *phase acceptance 2026-07-19: `cargo run` prints 3 real Event JSON lines + exits 0*

**Acceptance:** P5.3 checks green; kernel round-trip prints real `Event` JSON (floats and all) and exits 0.

**Gotcha:** `serde_json` without `std` still needs `alloc`. Float formatting is where no_std serde_json surprises live — that's why P5.4 names the float variants explicitly.

---

## Phase 6 — Agent core on metal: state fold + goal-driver semantics + scripted inference

**Objective:** the actual point — the colony's state semantics and goal lifecycle running on bare metal, narrated in protocol events.

- [x] P6.1 `agent-core/` crate + SYNC-COPY `state.rs` @ pin (commit `626802a`): wire-invisible adaptations ledgered in the file header (types import, `Map` alias, alloc imports incl. `String` for field types, env-test `Map::new()` fix); all 12 upstream unit tests green on host
- [x] P6.2 Goal driver implemented against a fresh source read of `goal.rs` (exact details verified: `Done` → `""`, budget exhaustion → `Done` + `"step budget reached"`, missing verdict defaults to `Continue(None)`, `max_steps` clamps `1..=100`); fidelity map documented in `agent-core/src/goal.rs` header. **Found + fixed via dedicated vendor commit:** ID newtypes needed `PartialOrd, Ord` derives for the BTreeMap side (see Changelog)
- [x] P6.3 `Inference` trait — **refinement vs the sketch here:** returns `TurnResult { Completed(Verdict) | Stalled }` rather than bare `Verdict`, because stall is a turn-engine observation, not an LLM verdict (mirrors upstream's two channels: `goal_step` vs watchdog). `ScriptedInference` + const transcripts `SCRIPT_SUCCESS` / `SCRIPT_STALL`; an exhausted script reads as stall (never hangs)
- [x] P6.4 Every transition emits `GoalStateChanged` with upstream's exact field set, one JSON object per line over UART
- [x] P6.5 Fold demo on metal: `UserPrompt → PluginUp → ApprovalPending → UserApproval` folded through the SYNC-COPY `SystemState::apply`, post-conditions asserted (`sessions=1 tools=2 approvals=0`), every folded event also emitted as wire JSON
- [x] P6.6 Kernel owns I/O only (`kernel/src/agent.rs` sink = serialize+println); `agent-core` pure; 9 driver host tests cover the FR-8 matrix (success walk, budget→Done, blocked reason, stall detail, script exhaustion, steer plumbing, clamp, state discipline + yolo=false, wire serialization) — *gate evidence 2026-07-19: success run exits 0 with the full `acting 1→2→3 → done` narrative; `--features fail-script` ends `failed`/`step stalled — no completion` and exits 2; hosttest 36 tests green*

**Acceptance:** run shows the fold assertions pass and the full transition narrative ending `Done`, exits 0; `--features fail-script` build ends `Failed` and exits nonzero; `cargo hosttest` green (state.rs upstream tests + goal-driver tests).

---

## Phase 7 — The loop, made honest

**Objective:** from "runs once" to a real cooperative agent loop with time.

- [x] P7.1 `kernel/src/time.rs`: CLINT `mtime` @ `0x200_BFF8` (10 MHz timebase), `mtimecmp0` @ `0x200_4000` for armed-`wfi` wakeups. No interrupts: `mstatus.MIE` stays off, no handler installed — but `mie.MTIE` must be set (see Changelog) so a pending MTIP can wake `wfi`
- [x] P7.2 Cooperative loop in `agent.rs`: `poll → (armed-wfi idle) → mtime watchdog`; agent-core gained `TurnResult::Pending` (in-flight turn, non-consuming in transcripts) + `GoalDriver::poll()` with persistent steer. `STEP_TIMEOUT` = 2 s emulated (scaled analog of upstream's 900 s); idle quantum 10 ms. Heartbeat: `loop: goal reached Done in N polls` — printed only on the success path, where N is structurally deterministic (every poll transitions); the fail path prints nothing timing-derived
- [x] P7.3 Verified from riscv-rt 0.18 `asm.rs` source: harts above `_max_hart_id` (default 0) jump to `abort`; surviving non-boot harts gate through `_mp_hook` before RAM init. With `-smp 1` + defaults, secondaries are parked at two layers
- [x] P7.4 `APEXOS-RV: goal done — halting` on `Done` → `exit_pass()`; `Failed` → `exit_fail(2)`
- [x] P7.5 Skipped as designed — filed in BACKLOG.md (no concrete need appeared; the cooperative loop carries v1)

**Acceptance:** deterministic full run — byte-identical UART log across two consecutive runs (diff them); exits 0. (Determinism note: the mtime-derived tick counts printed in heartbeat lines must be made deterministic — count ticks, not raw mtime values — or excluded from the log-diff by design; decide and record here.)

**Gate evidence (2026-07-19):** `run-qemu.sh` twice on the debug ELF → `cmp` byte-identical, exits 0/0; release ELF also exits 0 through the script. Determinism decision: poll counter only (never raw mtime), success-path-only. Timed negative gate: fail-script build exits 2 after **2107 ms** — ~2 s of genuine armed-`wfi` idle before the watchdog stalls the quiet step. Host suite: 38 tests green (incl. new Pending/steer-persistence coverage).

---

## Phase 8 — Verification harness & docs

**Objective:** one-command proof, and the repo tells its own story.

- [x] P8.1 `scripts/run-qemu.sh` — landed during P7 (it *is* the determinism instrument); timeout, tee, marker greps, exit propagation
- [x] P8.2 **Cross-repo test ✓ — the G7 crown jewel achieved:** `xtest/` with the renamed pristine git dep @ the pin; fixture = checked-in release-kernel capture (17 lines, 11 JSON frames). Three tests: every frame parses with upstream's unmodified types; the last frame is `Done` at step 3/8 with `yolo:false`; the goal walk equals the upstream lifecycle `Acting(1,2,3) → Done`. Both `apexos-protocol`s (vendored no_std producer, pristine upstream consumer) compile side by side in one graph
- [x] P8.3 `.github/workflows/ci.yml`: `host-tests` (hosttest) + `metal-boot` (release build + `run-qemu.sh`), with the Ubuntu qemu package-split fallback baked in
- [x] P8.4 `BACKLOG.md` written (mesh, real board, multi-hart, operator paths, persona, upstream PRs); `README.md` fleshed out with the sample run + layout map; PRD §12 Q3 resolved in place
- [x] P8.5 All boxes checked with evidence; tagged `v1.0.0`

**Acceptance:** fresh-clone Definition-of-Done walkthrough from PRD §13 passes end to end.

**Gate evidence (2026-07-19):** DoD satisfied — release build boots, streams the deterministic narrative, prints `APEXOS-RV: goal done — halting`, exits 0 via sifive_test (`run-qemu.sh` exit 0); byte-identical two-run diff; `cargo hosttest` **41/41** including the cross-repo suite (pristine upstream fetched from GitHub @ `676aa38` parsed every metal frame). v1 complete.

---

---

# v2 — the mesh arc (PRD §14)

Same rules, same gates. The kernel becomes a client of the colony's documented `/ws` contract; nothing upstream changes. Mock-gateway runs are the gates (D13); the live colony is the demo.

## Phase 9 — NIC bring-up: virtio-net on virtio-mmio

**Objective:** the kernel owns a network device.

- [x] P9.1 Pinned `virtio-drivers 0.13.0`; `Hal`/`MmioTransport`/`VirtIONet` read from crate source. Modern MMIO chosen: runner carries `-global virtio-mmio.force-legacy=false`
- [x] P9.2 Runner + `run-qemu.sh` carry the netdev flags with the explicit MAC; v1 flow byte-identical except the banner
- [x] P9.3 `kernel/src/net/mod.rs`: `RvHal` (static 256 KiB page-aligned DMA arena, atomic bump, **leak-by-design** dealloc — the NIC has kernel lifetime; identity share/unshare since no MMU ⇒ virt==phys), 8-slot probe, `VirtIONet<RvHal, MmioTransport, 16>` with 2 KiB buffers. Rule 3 roster grew `net/`
- [x] P9.4 Banner emitted before the fold demo — *gate evidence 2026-07-19: byte-identical two-run diff, exits 0/0, hosttest 41 green, module compiled clean on the first build*

**Acceptance:** `run-qemu.sh` exits 0 with the net banner present; two-run byte-identical diff still holds; hosttest unaffected. **Met 2026-07-19.**

## Phase 10 — TCP: smoltcp in the cooperative loop

**Objective:** real sockets, still no interrupts.

- [x] P10.1 Pinned `smoltcp 0.13.1` (no_std features: medium-ethernet/proto-ipv4/socket-tcp/alloc); `Device`/token traits read from source (Rx consume takes `&[u8]` in 0.13); glue in `net/smol.rs` — the standard raw-pointer token pair (rx/tx borrow the same NIC, consumed sequentially on one hart, SAFETY-commented); `mtime`/10k → `Instant::from_millis`
- [x] P10.2 Static slirp config (`10.0.2.15/24` → `10.0.2.2`), **fixed `random_seed`** for deterministic ISNs; armed-wfi 1 ms quanta between polls (same no-interrupt discipline)
- [x] P10.3 `xtest/src/bin/mockd.rs` echo mode (std-only, one connection, deterministic output)
- [x] P10.4 `net/smoke.rs` behind `net-smoke`: connect → ping → verify echo bytes → `net: tcp echo ok` → exit 0; 10 s mtime timeout panics visibly
- [x] P10.5 `scripts/run-mesh-qemu.sh` (mode → marker table: echo/ws/llm; builds + supervises mockd, trap-kills on exit)

**Acceptance:** mocked echo run exits 0 deterministically; plain run (no feature) still byte-identical with v1+banner. **Met 2026-07-19:** echo gate exit 0, two echo runs byte-identical, plain run byte-identical to the P9 baseline, hosttest 41 green, first build clean.

## Phase 11 — WebSocket: the gateway contract

**Objective:** speak `/ws` exactly as documented (upstream CLAUDE.md §agentd WebSocket protocol).

- [x] P11.1 Pinned `embedded-websocket 0.9.5` (API read from source: `client_connect`/`client_accept` with `HttpHeaderIncomplete` as the accumulate signal; `ReadFrameIncomplete` likewise for frames). `DetRng` (xorshift64*, fixed seed) for masking — deterministic ws bytes, per doctrine; RFC 6455 masking needs no crypto strength. Bearer token via build-time `APEXRV_TOKEN` (`option_env!`), absent = disabled
- [x] P11.2 Control frames landed in **`agent-core/src/gateway.rs`** (not the kernel — they're pure wire logic, and agent-core is host-tested): `GatewayControl::SessionInit` in, `ClientControl::Hello` out, shapes locked by tests against the doc's exact JSON. agent-core gained no_std serde/serde_json as real deps
- [x] P11.3 `mockd` ws mode via `tungstenite 0.30`: accept upgrade, push `session_init` (the gateway talks first), serve until close
- [x] P11.4 `net/mesh.rs`: `Mesh { establish, pump, send_text, next_text (answers pings), close_clean }` — machinery P12 reuses; smoke prints `mesh: session 42 established` and closes cleanly

**Acceptance:** mocked ws run exits 0 deterministically; frame shapes match the upstream doc verbatim. **Met 2026-07-19:** ws gate exit 0, two runs byte-identical (deterministic masking proven), plain run byte-identical to baseline, hosttest 43 green (+2 gateway shape locks), third consecutive first-build-clean network module. (The no-`session`-field mock assertion moves to P12.3 where `user_prompt` is first sent.)

## Phase 12 — MeshInference: live inference over the mesh

**Objective:** the thesis, completed — a goal on metal driven by a remote model.

- [x] P12.1 Split as designed: `step_directive` + `parse_goal_step` live in agent-core (host-tested: directive content, full verdict grammar incl. unknown-status→None→`Continue(None)` mirroring upstream, trailing-line-wins); `ClientFrame::UserPrompt { text }` shape-locked (no `session`, no `images`); kernel `MeshInference` owns the socket via P11's `Mesh` (+ new non-blocking `try_next_text` with cross-call fragment accumulation)
- [x] P12.2 In-flight turns return `Pending`; `drive_goal` is now generic over `Inference` (scripted and mesh share the identical loop + watchdog); `STEP_TIMEOUT` overridable at build time via `APEXRV_STEP_TIMEOUT_SECS` (mirrors upstream's `GOAL_STEP_TIMEOUT_SECS`), default 2 s for mock CI
- [x] P12.3 `mockd` llm mode (3 turns: continue/continue/done, two deltas + `turn_complete` each, **contract asserts on every inbound frame**: `user_prompt` with no `session`/`images`) + silent mode (one delta, then nothing)
- [x] P12.4 Gates **met 2026-07-19**: llm run `Acting 1→2→3 → Done` + `loop: goal reached Done over the mesh` + marker, exit 0, byte-identical twice; silent run `Failed`/`step stalled — no completion` exit 2 after ~2 s of real waiting — the watchdog's stall detail earned over TCP. Plain baseline untouched; hosttest 46
- [x] P12.5 `scripts/run-live.sh` written (gateway IP/port/token/step-timeout via build-time env; `10.0.2.2` = agentd on this host; captures `docs/live-run-<date>.log`). Live transcript + README/PRD note + `v2.0.0` tag land with the live-colony run

**Acceptance:** PRD §14 v2 Definition of Done end to end.

---

## Appendix A — Reference scaffolding

> **Placeholders, not gospel.** Versions looked up 2026-07-19; `riscv-rt`/`riscv`/`embedded-alloc` APIs and linker conventions drift — riscv-rt 0.18 in particular differs from the 0.12-era tutorials all over the internet. P1.3 pins real versions; when a snippet fights the pinned version's docs, **the docs win** — note the delta in the Changelog.

**Root `Cargo.toml`**
```toml
[workspace]
members         = ["kernel"]   # + "vendor/apexos-protocol" (P5), "agent-core" (P6), "xtest" (P8)
default-members = ["kernel"]
resolver        = "2"

[workspace.package]
version    = "0.1.0"
edition    = "2021"            # match upstream ApexOS-RS
license    = "Apache-2.0"
authors    = ["buckster123 (André)"]
repository = "https://github.com/buckster123/ApexOS-RV"

[workspace.dependencies]
# Mirrors upstream ApexOS-RS root so the pristine vendored protocol builds unchanged (P5.1).
serde      = { version = "1", features = ["derive"] }
serde_json = "1"
```

**Root `.cargo/config.toml`**
```toml
[build]
target = "riscv64gc-unknown-none-elf"

[target.riscv64gc-unknown-none-elf]
# riscv-rt 0.18 `memory` feature: generated link.x INCLUDEs memory.x via link-search.
rustflags = ["-C", "link-arg=-Tlink.x"]
# -m 256M > memory.x 128M on purpose — QEMU places the DTB above our claimed RAM.
runner = "qemu-system-riscv64 -machine virt -cpu rv64 -smp 1 -m 256M -bios none -nographic -serial mon:stdio -kernel"

[alias]
# Host-side testing/checking (agent-core, vendored protocol, xtest). Bare `cargo test`
# would cross-compile for RISC-V and fail — use these. Host triple is this box's.
hosttest  = "test  --workspace --exclude apexos-rv-kernel --target x86_64-unknown-linux-gnu"
hostcheck = "check --workspace --exclude apexos-rv-kernel --target x86_64-unknown-linux-gnu"
```

**`kernel/Cargo.toml`**
```toml
[package]
name    = "apexos-rv-kernel"
version.workspace = true
edition.workspace = true

[dependencies]
riscv          = { version = "0.16", features = ["critical-section-single-hart"] }  # feature confirmed @ 0.16.1
riscv-rt       = { version = "0.18", features = ["memory"] }  # memory: link.x INCLUDEs memory.x
embedded-alloc = "0.7"
# Phase 5+:
# apexos-protocol = { path = "../vendor/apexos-protocol", default-features = false, features = ["alloc"] }
# serde_json      = { version = "1", default-features = false, features = ["alloc"] }
# Phase 6+:
# apexos-rv-agent-core = { path = "../agent-core" }

[features]
panic-test  = []
fail-script = []
```

**`kernel/build.rs`**
```rust
use std::{env, fs, path::PathBuf};
fn main() {
    let out = PathBuf::from(env::var("OUT_DIR").unwrap());
    fs::copy("memory.x", out.join("memory.x")).unwrap();
    println!("cargo:rustc-link-search={}", out.display());
    println!("cargo:rerun-if-changed=memory.x");
    println!("cargo:rerun-if-changed=build.rs");
}
```

**`kernel/memory.x`** — QEMU `virt`, `-bios none` ⇒ we own RAM from its base. LENGTH must stay **below** the runner's `-m`: riscv-rt puts `.stack` at the region top, and QEMU needs headroom above the image for the DTB.
```text
MEMORY
{
  RAM : ORIGIN = 0x80000000, LENGTH = 128M
}

REGION_ALIAS("REGION_TEXT",   RAM);
REGION_ALIAS("REGION_RODATA", RAM);
REGION_ALIAS("REGION_DATA",   RAM);
REGION_ALIAS("REGION_BSS",    RAM);
REGION_ALIAS("REGION_HEAP",   RAM);
REGION_ALIAS("REGION_STACK",  RAM);

/* riscv-rt symbols — names/semantics: confirm against the pinned 0.18 docs */
_heap_size = 0x100000;        /* 1 MiB, if using the linker-provided heap route  */
/* _max_hart_id = 0;             default; raise only when -smp > 1 (Phase 7+)    */
/* _hart_stack_size = 0x10000;   per-hart stack if the default proves too small  */
```

**`kernel/src/main.rs`** (shape by end of Phase 4)
```rust
#![no_std]
#![no_main]

extern crate alloc;

mod heap;
mod qemu;
mod uart;

use riscv_rt::entry;

#[entry]
fn main() -> ! {
    println!("apexos-rv: hart 0 online");
    heap::init();
    let s = alloc::format!("alloc ok: {}", 42);
    println!("{s}");
    // Phase 6+: state fold assertions, then the goal loop
    qemu::exit_pass()
}

#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    println!("KERNEL PANIC: {info}");
    qemu::exit_fail(1)
}
```

**`kernel/src/uart.rs`** (core of it)
```rust
use core::fmt;

const UART0: *mut u8 = 0x1000_0000 as *mut u8; // NS16550A on QEMU virt
const LSR: usize = 5;
const LSR_THRE: u8 = 1 << 5;

pub fn putb(b: u8) {
    // SAFETY: UART0 is the QEMU-virt NS16550A MMIO base; byte-wide volatile
    // access to THR/LSR is the device contract. Single-hart (D4) ⇒ no races.
    unsafe {
        while UART0.add(LSR).read_volatile() & LSR_THRE == 0 {}
        UART0.write_volatile(b);
    }
}

pub struct Writer;
impl fmt::Write for Writer {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for b in s.bytes() {
            if b == b'\n' { putb(b'\r'); }
            putb(b);
        }
        Ok(())
    }
}
// + print!/println! macros wrapping Writer in critical_section::with(...)
```

**`kernel/src/qemu.rs`**
```rust
const SIFIVE_TEST: *mut u32 = 0x0010_0000 as *mut u32;

pub fn exit_pass() -> ! {
    // SAFETY: sifive_test MMIO on QEMU virt; 0x5555 = FINISHER_PASS.
    unsafe { SIFIVE_TEST.write_volatile(0x5555) };
    loop { riscv::asm::wfi(); }
}

pub fn exit_fail(code: u16) -> ! {
    // SAFETY: as above; (code << 16) | 0x3333 = FINISHER_FAIL with exit code.
    unsafe { SIFIVE_TEST.write_volatile(((code as u32) << 16) | 0x3333) };
    loop { riscv::asm::wfi(); }
}
```

**`scripts/run-qemu.sh`**
```bash
#!/usr/bin/env bash
set -euo pipefail
ELF="${1:?usage: run-qemu.sh <path-to-elf> [log]}"
LOG="${2:-/tmp/apexos-rv-uart.log}"
timeout 30s qemu-system-riscv64 -machine virt -cpu rv64 -smp 1 -m 256M \
  -bios none -nographic -serial mon:stdio -kernel "$ELF" | tee "$LOG"
# timeout kills a hang (exit 124); sifive_test propagates pass/fail otherwise
grep -q "apexos-rv: hart 0 online" "$LOG"
grep -q "APEXOS-RV: goal done — halting" "$LOG"
```

**`xtest` cross-repo dependency (P8.2)**
```toml
[dev-dependencies]
upstream-protocol = { package = "apexos-protocol", git = "https://github.com/buckster123/ApexOS-RS.git", rev = "676aa3870ad7e2b469be1dcaec23498c943491a9" }
serde_json = "1"
```

## Appendix B — Claude Code kickoff prompt

> Read CLAUDE.md, PRD.md and PLAN.md. We are executing **Phase N** of the plan. Start by restating the phase's tasks and acceptance gate, verify the previous phase's gate still passes, then propose your implementation plan for this phase before touching files. Follow the hard rules in CLAUDE.md — especially the vendoring/provenance discipline and the wire-format invariant. Stop at the acceptance gate and show me the verification output (commands + exit codes) before checking any boxes.

Start it in plan mode; on approval, auto mode (or accept-edits) fits the build-fix loops. For Phase 5 (vendoring + the upstreamable patch), prefer accept-edits and read those diffs yourself.

## Appendix C — Debugging crib

- Halted start for GDB: append `-s -S` to the QEMU command; then `gdb-multiarch target/riscv64gc-unknown-none-elf/debug/apexos-rv-kernel -ex 'target remote :1234'` → `break main`, `continue`, `info registers`, `x/8i $pc`.
- Instruction trace when truly lost: `-d in_asm,int -D /tmp/qemu.log` (huge; use with a quick timeout).
- Nothing on serial, ever: 90% linker — `readelf -h` (entry `0x80000000`?), `readelf -S` (sections inside RAM?), then confirm rustflags actually applied (`cargo build -v` shows the link args).
- Prints then wedges: usually a trap with no handler — riscv-rt 0.18's default trap behavior is the first thing to read; adding an early exception hook that prints `mcause`/`mepc` pays for itself immediately.
- Ctrl-A X exits `-nographic` QEMU; Ctrl-A C toggles the monitor.

## Changelog / deviations

- 2026-07-19 — v2: plan re-homed from `metal/`-inside-ApexOS-RS to this standalone repo. Corrections from source review @ `676aa38`: (1) `state.rs` = `SystemState` event-fold, goal lifecycle lives in `goal.rs` → Phase 6 restructured (SYNC-COPY the fold, reimplement driver *semantics*); (2) `Planning`/`Reflecting` unused upstream → scripted walk now mirrors the real `Acting→Done/Blocked/Failed` lifecycle; (3) `.cargo/config.toml` moved to repo root with `default-members` + host aliases (config-discovery trap); (4) versions refreshed (riscv-rt 0.18.0, riscv 0.16.1, embedded-alloc 0.7.0). Root-workspace guardrail replaced by vendoring/provenance discipline. Phase 0 partially complete (P0.1, P0.4).
- 2026-07-19 — Phase 0 nearly closed (gdb 17.1 ✓; found the Ubuntu `qemu-system-riscv` package split, P0.2 corrected). Added `docs/resources.md` + vendored goal-driver design doc (reference copy); cerebro-cortex continuity conventions added to CLAUDE.md; repo published to GitHub (public).
- 2026-07-19 — P1: riscv-rt 0.18 linking differs from Appendix A snippets (recorded per rule 6): the `memory` crate feature makes the generated `link.x` INCLUDE our `memory.x` from the link-search path (cortex-m-rt style), so rustflags carry a single `-Tlink.x` — the old "memory.x before link.x" two-flag order is obsolete. `memory.x` symbol contract unchanged (REGION_ALIAS, `_heap_size`, `_max_hart_id` all present in 0.18's link.x.in). First build green; ELF entry `0x80000000` confirmed via readelf (P2.3 pre-verified). `cargo hostcheck` currently exits 101 with a benign "workspace has no members" error (kernel excluded, no host members until P5) — acceptance interpreted accordingly.
- 2026-07-19 — v2/P12 (mock gates) ✓ — a goal ran over the mesh: every step a real `user_prompt`→`agent_text`→`turn_complete` round trip, verdicts via the D14 `GOAL_STEP:` line, byte-identical llm runs, and the silent-mock negative gate proved the P7 watchdog stalls a *real* network turn (`step stalled — no completion`, exit 2). `drive_goal` generic over `Inference` — scripted CI and live mesh share one loop. v2.0.0 tag awaits the live-colony transcript (run-live.sh).
- 2026-07-19 — v2/P11: WebSocket ✓ — `mesh: session 42 established`; tungstenite (a strict real-world ws peer) accepted the kernel's RFC 6455 handshake; deterministic masking RNG held byte-identical runs; clean close handshake both sides. Control frames live in agent-core (host-tested wire logic), not the kernel — boundary discipline paying off.
- 2026-07-19 — v2/P10: TCP on metal ✓ — `net: tcp echo ok` through virtio-net → smoltcp → slirp → host mock, polled with armed-wfi 1 ms quanta, fixed smoltcp `random_seed` for deterministic ISNs. `main` split into `main`/`normal_flow` so diverging feature gates stay warning-free. Second consecutive first-build-clean network module.
- 2026-07-19 — v2/P9: NIC up ✓ — `net: virtio-net up mac=52:54:00:0b:ee:f1`. virtio-drivers 0.13 over modern virtio-mmio (force-legacy=false), static-arena Hal with identity share (no MMU). First-build-clean; determinism gate held with the new banner.
- 2026-07-19 — **v1.0.0** — P8 ✓ and Definition of Done met: all eight phases gated with evidence in a single day (project bootstrap → docs v2 → first boot → diagnostics → heap → protocol vendored+gated → agent core → honest loop → cross-repo proof). 41 host tests; kernel exits by its own report; wire compat proven against pristine upstream across the repo boundary. Upstream PR material banked in `vendor/apexos-protocol/UPSTREAM.md` + BACKLOG §Upstream.
- 2026-07-19 — P7: the honest loop ✓ — earned gotcha: **riscv-rt boots with `mie = 0`, and `wfi` wakes only on per-source-enabled pending interrupts** — without `mie.MTIE` the armed wakeup never fires and idle sleeps forever (first run hung at Acting(2); `time::init()` sets MTIE once, global `mstatus.MIE` stays off, no trap ever taken). `unsafe` roster in CLAUDE.md rule 3 grew `time.rs`. Driver API grew `TurnResult::Pending` + `poll()` so the negative path is a *time-measured* watchdog stall (2107 ms observed), not a scripted self-report.
- 2026-07-19 — P6: agent core on metal ✓ — SYNC-COPY state fold asserted on metal; goal driver reimplemented from a fresh `goal.rs` source read (exact detail strings + `Continue(None)` default + clamp captured in the fidelity map). Two recorded refinements: (1) `Inference` returns `TurnResult{Completed(Verdict)|Stalled}` — stall is turn-engine observation, not verdict (mirrors upstream's two channels); (2) **vendor patch addendum (dedicated commit):** ID newtypes gain `PartialOrd, Ord` — they key `SystemState` maps and BTreeMap requires `Ord`; wire-invisible, caught by the compiler exactly as the two-sided test strategy intended, added to the upstream-PR ledger.
- 2026-07-19 — P5: protocol on metal ✓ — two-commit vendor discipline executed; full test suite green under BOTH feature gates on host; vendored crate compiled for riscv64gc first try; 3 colony events round-tripped on metal (floats + Value trees included) and streamed line-delimited over UART, exit 0. Bonus find: upstream ships `tests/redteam.rs` (adversarial no-panic corpus) — vendored along, and its "hostile frame can't kill the node" property is exactly what the post-v1 mesh path needs.
- 2026-07-19 — P4: heap ✓ — static-array route; `embedded-alloc 0.7` slimmed to `llff` only (drops the rlsf/tlsf deps; D5's smallest-allocator rationale). Tiny-heap proof implemented as a repeatable `tiny-heap` feature (P3.5 precedent) instead of a temporary edit: OOM panics readably through the reporting handler and exits 1 — never hangs.
- 2026-07-19 — P3: diagnostics floor ✓ — normal run exits **0** (FINISHER_PASS; no timeout scaffolding needed anymore), `--features panic-test` prints the full panic (message + `main.rs:15:5` location) and exits **1**. `critical-section = "1.2"` added as a direct kernel dep (the API crate for the print lock; Appendix A's macro sketch implied it). Panic path kept cfg-gated/inert rather than deleted — re-runnable negative test for future CI.
- 2026-07-19 — P2: first boot ✓ — `apexos-rv: hart 0 online` over the NS16550A, then parks in `wfi`. Earned gotcha: riscv-rt links `.stack` to the **top** of memory.x's RAM, so QEMU's `-m` must be *larger* than the claimed region or the DTB has nowhere to go (`No enough memory to place DTB after kernel/initrd`). The v1 draft's 256M/128M "mismatch" was load-bearing; restored deliberately (comments in memory.x + config.toml explain). Appendix A snippets updated to 0.18 reality (single `-Tlink.x`, `memory` feature, `-m 256M`).
