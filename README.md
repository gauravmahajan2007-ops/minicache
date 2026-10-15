# MiniCache (Rust)

A minimal in-memory TTL key-value cache with a simulated clock — built as the
first end-to-end test of the **Hermes Systems Lab** methodology:

```
spec → capabilities → invariants → independent reference model
     → implementation → property-based tests → repair workflow
```

## Layout

- `SPEC.md` — behavioral specification (expiry boundary, overwrite-reset, delete, invalid TTL)
- `DESIGN.md` — capabilities + dependencies, invariants (I1–I8), acceptance criteria
- `src/lib.rs` — the implementation (`MiniCache<K, V>` generic cache, `SimulatedClock`)
- `tests/` — unit tests, `proptest` property tests, repair/regression tests, and a test-only faulty fixture
- `tests/common/mod.rs` — the independent reference model (oracle)
- `counterexamples/` — committed minimal counterexample from the repair workflow
- `evidence/` — authentic failing-test output from the repair demonstration
- `REPORT.md` — end-to-end report with real test evidence and the readiness decision
- `KVCACHE_INTEGRATION_PLAN.md` — plan for evolving MiniCache toward NVIDIA KV-cache integration

## Run the tests

```sh
cargo test
```

Expected: **16 passed, 0 failed** (11 unit + 2 property + 3 repair).

> **Note:** this was developed on a host that lacked `libc6-dev`. A local
> `.cargo/config.toml` workaround (deliberately **not committed**) pointed the
> linker at a user-local sysroot. On a normal host with `libc6-dev` installed,
> `cargo test` works out of the box.
