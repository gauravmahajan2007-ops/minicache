# KvPool — Capabilities, Invariants, Acceptance Criteria

## 1. Capabilities and dependencies

| Capability | Depends on | Notes |
|---|---|---|
| C1. Contiguous byte buffer | std (`Vec<u8>`) | `CpuPool` |
| C2. Block read/write/clear | C1 | offset = block * block_size |
| C3. Bounds + length validation | C1 | InvalidBlock / BadLength |
| C4. Device abstraction | — | `KvPool` trait |
| C5. CUDA device pool | CUDA runtime | `CudaPool`, `#[cfg(feature="cuda")]` |
| C6. Property-based harness | proptest (dev) | round-trip/isolation/clear |

Dependency graph: `C1 → C2, C3`; `{C2,C3} → CpuPool`; `CpuPool → C4 (impl)`;
`C5 → C4 (impl)`; `{CpuPool} → C6`.

## 2. Invariants (see SPEC_POOL.md §5)

I1 round-trip · I2 clear · I3 isolation · I4 bounds · I5 length.

## 3. Acceptance criteria (testable)

1. All unit tests pass — I1–I5 explicitly.
2. All property tests pass — random data/blocks round-trip, isolation, and clear.
3. Integration test passes — `BlockManager` + `CpuPool` compose (bytes survive
   release/acquire; clear-on-invalidate).
4. `cargo check --features cuda` type-checks the `CudaPool` (link/run requires a
   GPU, out of local scope).
5. Repair workflow demonstrated with real red output, saved counterexample, fix,
   and green rerun.
6. Readiness decision recorded in `REPORT_POOL.md`.

## 4. What "done" does NOT mean

A pool that round-trips a single block is insufficient. Isolation (writing one
block never corrupts another) and clear-on-free are the real gates, because
off-by-one offset or stale-data bugs live exactly there.
