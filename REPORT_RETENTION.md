# BlockManager (Pass 1) — End-to-End Report

Systems Lab Pass 1: the retention/eviction policy core toward NVIDIA KV-cache
integration. Date: 2026-10-15 · Toolchain: rustc/cargo 1.99.0, proptest 1.12.0.

---

## 1. What was built

`BlockManager` (`src/block_manager.rs`) + retention types (`src/retention.rs`) —
a fixed-size block pool with two lifecycles and TensorRT-LLM-mirroring retention
semantics:

- **Priority** `0..=100` (default **35**) and **duration-TTL** — a reusable
  block's priority reverts to default once `now >= expires_at`, without evicting.
- **Prioritized-LRU eviction** — lowest effective priority first, LRU tie-break.
- **Two lifecycles** — `Active { refcount }` vs `Reusable { priority, expires_at,
  last_used }`; `release` at refcount 0 → reusable (never free); `acquire`
  promotes reusable → active; `invalidate` force-frees (idempotent).
- **Never evict active** — allocation returns `None` (OOM) when all blocks are
  active.

The existing TTL `MiniCache` remains as the tested foundation; `BlockManager`
is its evolution. No hardware, no CUDA, no real time — simulated clock + small
pool force deterministic eviction.

## 2. Specification & design

`SPEC_RETENTION.md` (types, operations, effective-priority rule, invariants
I1–I9) and `DESIGN_RETENTION.md` (capabilities, invariants, acceptance criteria).
`KVCACHE_INTEGRATION_PLAN.md` now records the approved scope and frozen
decisions (radix prefix reuse and the CUDA byte-pool seam are deferred to Pass 2).

## 3. Reference model

`tests/common/block_manager_ref.rs` — an independent, naive oracle
(`ReferenceBlockManager`) written from the spec, plus a deliberately faulty
fixture (`FaultyBlockManager`).

## 4. Tests actually executed

**`cargo test` — 34 passed, 0 failed; `cargo clippy --all-targets -- -D warnings`
clean; `cargo fmt --check` clean.**

- `tests/retention_unit.rs` (14) — I1–I9: duration revert at exact expiry,
  priority-then-LRU eviction, LRU tie-break, never-evict-active, refcount,
  reuse-resets-retention, invalidate idempotency, invalid priority/duration.
- `tests/retention_properties.rs` (1) — proptest: impl ≡ oracle over generated
  allocate/release/acquire/invalidate/advance sequences, full-state agreement
  (state, effective priority, refcount, expires_at, eviction candidate,
  allocation result) after every step.
- `tests/retention_repair.rs` (3) — permanent duration-revert boundary guard,
  faulty-fixture defect documentation, committed-counterexample check.
- Existing `tests/unit.rs` (11), `test_properties.rs` (2), `test_repair.rs` (3)
  — the TTL cache suite, still green.

## 5. Generated-test results (proptest)

```
BlockManager:  implementation_agrees_with_reference   with_cases(500)   PASS
TTL cache:     implementation_agrees_with_reference   with_cases(300)   PASS
TTL cache:     invalid_ttl_agreement                  with_cases(200)   PASS
```

1000 generated cases total, 0 failures — implementation and reference model
agree on every sequence.

## 6. Repair workflow — counterexample and repair evidence

`FaultyBlockManager` injects one defect: the duration-revert comparison uses `>`
instead of `>=`, so a block's priority fails to revert *at exactly* its expiry
time (violates invariant I2 / SPEC_RETENTION.md §3).

**Detection (authentic red, `evidence/repair-retention-red-output.txt`):**

```
---- duration_revert_against_faulty_fixture_detects_defect stdout ----
assertion `left == right` failed
  left: Some(90)   # stale priority
 right: Some(35)   # DEFAULT_PRIORITY expected
test result: FAILED. 0 passed; 1 failed  (exit 101)
```

**Counterexample** (`counterexamples/duration-revert-off-by-one.json`): minimal
sequence — allocate, release(priority=90, duration=10ms), advance(10),
effective_priority → observed 90 (buggy), expected 35.

**Fix:** the correct comparison is `now >= expires_at` (implemented in
`BlockManager`). The transient red test was removed and the full suite reran
green (**34 passed**), with `duration_revert_boundary_regression` locking the
boundary.

## 7. Lessons captured (this run)

- `f64` is `PartialEq` but not `Eq`, so any `#[derive(Eq)]` on a type containing
  `f64` (or `Option<f64>`) fails to compile — drop `Eq`. (Hit on `BlockState`,
  `RefBlockState`, `RetentionError`.)
- `NaN != NaN`: `assert_eq!(Err(InvalidDuration(NaN)), ...)` can never pass —
  use `.is_err()` / `matches!` for non-finite values. (Repeated from the TTL
  cache run; this time caught by the test itself, not me.)
- A redundant inherent `default()` alongside `impl Default` trips
  `clippy::should_implement_trait`; keep only the trait impl.
- The oracle's `best.map_or(true, …)` should be `best.is_none_or(…)`.

## 8. Readiness decision

**READY (Pass 1).** Every acceptance criterion in DESIGN_RETENTION.md §3 is met:
unit tests pass; property tests pass (impl ≡ oracle, 500 cases); eviction is
forced deterministically under simulated pressure; repair workflow demonstrated
with real red output, saved counterexample, fix, and green rerun.

Deferred (by approved plan): radix-tree prefix reuse and the CUDA byte-pool seam
(Pass 2).
