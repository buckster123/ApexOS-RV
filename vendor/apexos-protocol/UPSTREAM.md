# UPSTREAM — vendored `apexos-protocol`

| | |
|---|---|
| Source | https://github.com/buckster123/ApexOS-RS |
| Path | `apexos-protocol/` |
| Pin | `676aa3870ad7e2b469be1dcaec23498c943491a9` (main, 2026-07-15) |
| Vendored | 2026-07-19 — pristine, byte-identical (`diff -r` clean at copy time) |

## Files

- `Cargo.toml`, `src/lib.rs`, `tests/redteam.rs`, `README.md` — upstream, at the pin
- `LICENSE` — copy of the upstream repo-root Apache-2.0 text (the crate dir carries its own notice here)
- `UPSTREAM.md` — this file (local; not from upstream)

## Local patches

*(one entry per change: date — files — what/why — wire impact; the cumulative diff vs pristine is the future upstream PR)*

- 2026-07-19 — `Cargo.toml`, `src/lib.rs`, + new `tests/wire_compat.rs` — **the `no_std` gate**: features `default = ["std"]` / `std` / `alloc`; deps concretized with `default-features = false` (a workspace-inherited dep can't switch defaults off at the use site); `#![cfg_attr(not(feature = "std"), no_std)]` + `extern crate alloc`; `std::fmt` → `core::fmt`; `String`/`Vec` from `alloc` under `not(std)`; new `pub type Map<K, V>` (`HashMap` under std ⇄ `BTreeMap` under no_std) applied to the one map field `RegisterMcpServer.env`; one in-lib test's `std::collections::HashMap::new()` → `Map::new()` so the suite compiles under both gates. **Wire impact: none** — locked by `tests/wire_compat.rs` (semantic JSON equality) and the untouched wire-lock + redteam suites passing under both feature sets. The cumulative diff vs the pristine commit is the upstream PR.
- 2026-07-19 — `src/lib.rs` — ID newtypes (`SessionId`, `ActionId`, `GoalId`, `EvolutionId`, `PluginId`) additionally derive `PartialOrd, Ord`: they key `SystemState`'s maps, and the `no_std` side's `BTreeMap` requires `Ord` (found by the P6 agent-core build). **Wire impact: none** — derives don't affect serde representation; all suites stay green.
