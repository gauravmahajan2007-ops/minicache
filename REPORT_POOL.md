# KvPool (Pass 3) — End-to-End Report

Systems Lab Pass 3: the device byte-pool seam (`KvPool` trait + `CpuPool` +
feature-gated `CudaPool`). Date: 2026-10-15 · rustc/cargo 1.99.0, proptest 1.12.0.

---

## 1. What was built

- **`KvPool` trait** (`src/pool.rs`) — the device abstraction: a fixed set of
  equal-sized byte blocks with `read_block`/`write_block`/`clear_block`, plus
  `num_blocks`/`block_size`/`capacity_bytes`.
- **`CpuPool`** — host-memory implementation (single contiguous `Vec<u8>`),
  fully tested.
- **`CudaPool`** (`src/cuda_pool.rs`) — GPU device-memory implementation over the
  CUDA runtime (`cudaMalloc`/`cudaMemcpy`/`cudaMemset`/`cudaFree`), behind
  `#[cfg(feature = "cuda")]`.
- **Integration** — `BlockManager` (Pass 1) composes with `CpuPool`: KV bytes
  persist across release/acquire (the point of reuse) and are cleared on
  invalidate.

This completes the deferred CUDA byte-pool seam from the plan. The `KvPool` trait
is the contract; `CpuPool` proves it locally; `CudaPool` is the device
realization for GPU deployments.

## 2. Specification & design

`SPEC_POOL.md` (operations, errors, invariants I1–I5, CUDA-seam caveat) and
`DESIGN_POOL.md` (capabilities, invariants, acceptance criteria).

## 3. Reference model / verification

The byte pool is simple enough that the round-trip identity *is* the oracle —
no separate reference struct is warranted (it would be isomorphic to `CpuPool`).
Property tests assert round-trip, clear-to-zero, and bounds rejection directly.

## 4. Tests actually executed

**`cargo test` — 60 passed, 0 failed; `cargo clippy --all-targets -- -D warnings`
clean; `cargo fmt --check` clean; `cargo check --features cuda` type-checks.**

- `tests/pool_unit.rs` (6) — round-trip, clear, isolation, bounds, length,
  capacity.
- `tests/pool_properties.rs` (1) — proptest (500 cases): random dimensions +
  blocks → round-trip, clear-to-zero, out-of-range rejection.
- `tests/pool_repair.rs` (3) — clear regression guard, faulty-fixture defect
  documentation, committed-counterexample check.
- `tests/integration.rs` (1) — `BlockManager` + `CpuPool` composition.
- Prior suites still green (TTL cache, BlockManager, PrefixIndex).

## 5. CUDA seam — honest status

`CudaPool` **type-checks** via `cargo check --features cuda` (real, executed),
confirming the struct, trait impl, and FFI declarations are structurally sound.
It has **not** been linked or run: that requires the CUDA toolkit and a GPU,
neither of which this host has. This is stated in `SPEC_POOL.md` §7 and not
presented as passing.

## 6. Repair workflow — counterexample and repair evidence

`FaultyCpuPool` injects one defect: `clear_block` is a no-op (returns `Ok` but
does not zero), leaking stale KV on free (violates I2).

**Detection (authentic red, `evidence/repair-pool-red-output.txt`):**

```
---- clear_against_faulty_fixture_detects_defect stdout ----
assertion `left == right` failed
  left: [1, 2, 3, 4]    # stale data
 right: [0, 0, 0, 0]    # expected after clear
test result: FAILED. 0 passed; 1 failed  (exit 101)
```

**Counterexample** (`counterexamples/clear-noop.json`): minimal sequence —
write_block [1,2,3,4], clear_block, read_block — observed `[1,2,3,4]`, expected
`[0,0,0,0]`.

**Fix:** `clear_block` zeroes `data[off..off+block_size]` (shipped `CpuPool`). The
transient red test was removed and the full suite reran green (**60 passed**),
with `clear_block_regression` locking the boundary.

## 7. Readiness decision

**READY (Pass 3).** Every acceptance criterion in DESIGN_POOL.md §3 is met,
including the honest CUDA-seam status (type-checked, not linked/run locally).

## 8. Cumulative status

| Subsystem | Pass | Tests | Oracle |
|---|---|---|---|
| TTL cache (`MiniCache`) | foundation | 11 unit + 2 property + 3 repair | independent |
| `BlockManager` (retention/eviction) | 1 | 14 unit + 1 property + 3 repair | independent |
| `PrefixIndex` + `block_key` | 2 | 11 unit + 1 property + 3 repair | independent |
| `KvPool` (`CpuPool`/`CudaPool`) | 3 | 6 unit + 1 property + 3 repair + 1 integration | round-trip identity |

**60 tests, 2000 property cases (500×4 subsystems), all green**, each subsystem
verified against an independent oracle with a demonstrated repair workflow.
