# BlockManager — Capabilities, Invariants, Acceptance Criteria

## 1. Capabilities and dependencies

| Capability | Depends on | Notes |
|---|---|---|
| C1. Simulated clock | std | reused from MiniCache (`SimulatedClock`) |
| C2. Retention validation | std | priority ≤ 100; duration > 0 or None |
| C3. Block pool (fixed size) | std | `Vec<BlockState>` + free list |
| C4. Two lifecycles (Active/Reusable) | C1, C3 | refcount + retention |
| C5. Duration-TTL (priority revert) | C1, C3 | effective-priority rule |
| C6. Prioritized-LRU eviction | C1, C3 | priority asc, then last_used asc |
| C7. Reference model (oracle) | C1 | independent of C3–C6 |
| C8. Property-based harness | proptest (dev) | replays op sequences on C3 vs C7 |

Dependency graph: `C1 → C4, C5, C6`; `C2 → C4`; `{C3,C4,C5,C6} → BlockManager`;
`{C1} → C7`; `{BlockManager, C7} → C8`.

## 2. Invariants (see SPEC_RETENTION.md §6)

I1 default priority 35 · I2 duration revert · I3 prioritized LRU · I4 never evict
active · I5 release→reusable · I6 reuse resets retention · I7 invalidate
idempotent · I8 duration None = no revert · I9 monotonic recency.

## 3. Acceptance criteria (testable)

1. All unit tests pass — explicit coverage of I1–I9, eviction ordering, duration
   revert, refcount, invalidate, and invalid-input rejection.
2. All property tests pass — implementation and reference model agree on **every**
   generated operation sequence: block state, effective priority, refcount,
   eviction candidate, and allocation result after every step.
3. Simulated memory pressure (small pool) forces eviction deterministically.
4. Repair workflow demonstrated with real red output, a saved counterexample, a
   fix, and a green rerun.
5. Readiness decision recorded in `REPORT_RETENTION.md`.

## 4. What "done" does NOT mean

Unit tests alone are insufficient. The property-based agreement test and the
eviction-order regression test are the real gates — the subtle bugs (duration
revert, LRU tie-break, "never evict active") live exactly where unit tests
under-sample.
