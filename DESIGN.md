# MiniCache — Capabilities, Invariants, Acceptance Criteria (Rust)

## 1. Capabilities and dependencies

| Capability | Depends on | Notes |
|---|---|---|
| C1. Simulated clock | std only | deterministic, monotonic `f64` time |
| C2. TTL validation | std only | finite, `> 0` (type rejects non-`f64`) |
| C3. KV store with absolute expiry | C1, C2 | `HashMap<K, (V, f64)>`; `put` sets `expiry = now + ttl` |
| C4. Lazy eviction | C1, C3 | `get` removes expired entries on access |
| C5. Overwrite/reset semantics | C1, C3 | `put` replaces value *and* expiry |
| C6. Reference model (oracle) | C1, C2 | independent of C3/C4/C5; test-only |
| C7. Property-based harness | proptest (dev-dep) | replays generated op sequences on C3 vs C6 |
| C8. Repair/regression harness | std + proptest | faulty fixture + boundary regression |

Dependency graph: `C2 → C3`, `C1 → C3`, `C1 → C4`, `{C1,C2} → C6`,
`{C3,C6} → C7`, `{C3} → C8`.

## 2. Invariants (must always hold)

- **I1 (absolute expiry).** After `put(k, v, ttl)` at time `t`, the entry's
  expiry is exactly `t + ttl`.
- **I2 (liveness boundary).** `get(k)` returns `Some` iff `now < expiry`;
  returns `None` iff absent *or* `now >= expiry`.
- **I3 (overwrite resets TTL).** After `put(k, v2, ttl2)` at time `t2`, expiry is
  `t2 + ttl2`, independent of any prior entry for `k`.
- **I4 (delete idempotency).** After `delete(k)`, `get(k) == None`; repeating
  `delete(k)` is a no-op and never errors.
- **I5 (lazy eviction).** A `get` that observes expiry removes the entry; `size()`
  then no longer counts it.
- **I6 (monotonic clock).** `now()` never decreases.
- **I7 (invalid TTL no-op).** An invalid TTL returns `Err` and mutates nothing.
- **I8 (size == live count).** `size()` equals the number of keys with
  `now < expiry`.

## 3. Acceptance criteria (testable)

1. All ordinary unit tests pass (covers I2, I3, I4, I5, I6, I7, I8 explicitly).
2. All property-based tests pass: implementation and reference model agree on
   every generated operation sequence, including full-state comparison after
   every step.
3. The six required properties are explicitly verified:
   - key unreadable at/after expiry ✓
   - key readable before expiry ✓
   - delete makes key unavailable ✓
   - delete of missing key safe and repeatable ✓
   - overwrite follows documented TTL semantics ✓
   - impl agrees with reference model on generated sequences ✓
4. Repair workflow demonstrated with **real** failing output (a test detects a
   deliberately faulty fixture), a saved reproducible counterexample, a fix, and
   a green rerun of the regression test.
5. A readiness decision is recorded in `REPORT.md`.

## 4. What "done" does NOT mean

Passing unit tests is necessary but not sufficient. The property-based agreement
test and the boundary regression test are the real gates, because off-by-one
expiry and overwrite-reset bugs live exactly at the boundaries that unit tests
tend to under-sample.
