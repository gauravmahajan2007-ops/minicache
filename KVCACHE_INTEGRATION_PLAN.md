# Plan: Adopting MiniCache toward NVIDIA KV-cache integration

Status: **APPROVED — implementation in progress (Pass 1 below).**
Method: Systems Lab (spec → capabilities → invariants → reference model → impl → property tests → repair).

---

## 1. Context — what "NVIDIA KV cache" actually is

Grounded in TensorRT-LLM source (`cpp/tensorrt_llm/batch_manager/kvCacheManager.cpp`,
`docs/source/features/kvcache.md`) and the retention-config docs.

The NVIDIA KV cache is **not** a scalar key→value store. It is a **GPU tensor
memory manager** with a specific hierarchy:

```
Pool    = contiguous memory buffer (primary = GPU, secondary = CPU offload)
Block   = metadata handle; each block holds `tokens_per_block` tokens of K/V
          for a layer (fixed size, ~16–64 KB). blockId → pool index.
WindowBlockManager = pools+blocks per attention-window size
BlockManager       = all windows; allocation, reuse, eviction
```

Two distinct *lifecycles* coexist, and this is the crux:

1. **Active blocks** (in-flight sequences) — reference-counted
   (`mRefCount`, `mSchedulingRefCount`), freed on sequence end. This is
   **lifetime/refcount semantics, not TTL.**
2. **Reusable blocks** (prefix cache / offloaded tiers) — kept after a sequence
   finishes for reuse by later requests sharing a token prefix. This is where
   **TTL + priority eviction** lives.

The retention config (`KvCacheRetentionConfig`) is the exact overlap with
MiniCache's problem:

- `TokenRangeRetentionConfig(start, end, priority, duration)` — priority is
  **0–100**, default **35**.
- `duration_ms` = how long the priority level applies, measured **from the first
  time the block became available for reuse**. After it elapses, priority
  **reverts to default**. → **This is literally a TTL.**
- `decode_retention_policy` / `decode_duration_ms` for generated tokens.
- Eviction scheme: **prioritized LRU** — lowest priority evicted first; within a
  priority tier, least-recently-used.
- Reuse = **longest-prefix match over a radix tree** keyed by block/token hash;
  `block_key` hashing is a cross-language contract used by the router.

NVIDIA Dynamo adds **disaggregated prefill/decode** and a **KV-cache-aware
router** that consumes KV *events* (stored/removed/created/updated) and routes a
request to the worker already holding its prefix (transfers over NIXL/RDMA).
LMCache adds **cross-engine + multi-tier offload** (GPU→CPU→NVMe→remote).

## 2. Where MiniCache maps (and where it does not)

| Concern | MiniCache today | NVIDIA KV cache | Verdict |
|---|---|---|---|
| Key | scalar, hashable | token sequence → radix prefix | **extend** |
| Value | arbitrary | GPU K/V block handle | **abstract** |
| Expiry | absolute `now+ttl` | `duration_ms` from first-reusable | **≈ maps** |
| Overwrite | reset TTL | re-cache resets duration | **≈ maps** |
| Delete | idempotent remove | block invalidation | **≈ maps** |
| Eviction | TTL-only (size() counts live) | **priority-then-LRU** | **add** |
| Priority | none | 0–100, default 35, reverts on expiry | **add** |
| Active vs reusable | not modeled | refcount + retention | **add** |
| Device | in-memory dict | GPU tensors | **abstract/FFI** |

**The honest core insight:** MiniCache does *not* map to the whole KV cache. It
maps precisely to the **reuse-retention / eviction policy** of *reusable blocks*
— the one layer NVIDIA itself models with TTL (`duration_ms`) + priority +
prioritized-LRU. That is a coherent, hardware-agnostic, property-testable
subsystem, and it is the right thing to build first with Systems Lab rigor.

## 3. Integration concept (recommended scope)

Turn MiniCache into a **KV-cache retention/eviction policy core** that mirrors
TensorRT-LLM's retention semantics, behind a **device abstraction**, so:

- locally it runs against a **CPU buffer pool** (deterministic, no GPU);
- later it can bind to a **CUDA pool** (feature-gated, real GPU) or to
  TensorRT-LLM's C++ block manager via a thin connector (the LMCache pattern).

The core is *not* the GPU memory manager (that is TensorRT-LLM's C++ job, deeply
tied to CUDA + the engine). We build the **policy** as a correct, tested Rust
library and expose a clean seam for the device.

## 4. Needed pieces (what we would build)

- **P1 — Block/pool model.** `KvPool` trait (alloc/free a fixed-size block,
  primary/secondary tier). `CpuPool` (real, `Vec<u8>`-backed) + `CudaPool`
  (behind `#[cfg(feature="cuda")]`, cudarc-backed).
- **P2 — Retention entry.** `(priority: u8, expiry: absolute time, last_used)`
  per reusable block; overwrite resets expiry (reuse MiniCache semantics).
- **P3 — Prioritized-LRU eviction.** Under memory pressure: evict lowest
  priority first, LRU within tier; on `duration_ms` expiry, priority reverts to
  default (35) *without* evicting — exactly NVIDIA's behavior.
- **P4 — Radix-tree prefix reuse.** Key = token id sequence; longest-prefix
  match; block-key hash for router compatibility.
- **P5 — Two lifecycles.** Active blocks (refcount, freed on sequence end) vs
  reusable blocks (retention policy). Reuse MiniCache's tested delete/TTL core
  for the reusable side.
- **P6 — Simulated clock + simulated pressure.** Deterministic time and a small
  block pool so eviction is forced and reproducible.
- **P7 — Reference model.** Independent oracle encoding NVIDIA's exact retention
  semantics (priority 0–100, default 35, duration TTL, priority-then-LRU,
  radix prefix), written from spec.
- **P8 — Property + unit tests.** Replay generated op sequences (allocate/free/
  cache/expire/evict) against impl vs oracle; regression-guard the boundaries.

## 5. Missing pieces (what we cannot fully do here)

- **GPU hardware.** No CUDA device on this host. The `CudaPool` + any real
  TensorRT-LLM link can be *compiled* (feature-gated) but not *executed* locally.
- **TensorRT-LLM C++ block manager binding.** No official Rust binding; it is a
  large C++/CUDA codebase. Options: FFI to the C++ lib, or treat it as an
  external device via a connector (LMCache-style). This is an *integration seam*,
  not something to reimplement.
- **NIXL/RDMA transfer** and **Dynamo router** integration — out of scope for a
  first Systems Lab pass; they are consumers of the events our core would emit.
- **Real KV tensor data** — we model *block handles/metadata*, not the bf16/fp8
  tensors (that would require an actual model + CUDA).

## 6. How to test locally without much hardware

This is the key question, and the answer is: **the policy core is pure logic and
fully testable on CPU with a simulated clock and a simulated block pool.**

1. **CPU device backend** — `CpuPool` allocates blocks out of a `Vec<u8>`; no
   GPU, no CUDA driver, no model. The "value" is a block index + metadata.
2. **Simulated clock** — reuse MiniCache's `SimulatedClock`; `duration_ms`
   expiry tested at exact boundaries (deterministic, fast).
3. **Simulated memory pressure** — a pool sized to a few blocks, so eviction is
   forced on small operation sequences (cheap, deterministic).
4. **Property-based oracle test** — a reference model implements NVIDIA's exact
   semantics (priority-then-LRU + duration TTL + prefix radix); proptest replays
   random sequences and asserts exact agreement — zero hardware.
5. **Fake engine driver (optional)** — a tiny "sequence simulator" that
   allocates blocks for fake prefill/decode, exercising the active/reusable
   lifecycle without a model.
6. **CUDA path** — `#[cfg(feature="cuda")]` so CI (or any GPU box later) can
   compile-check and, when a GPU exists, run the same tests against `CudaPool`.
   Locally we build with the feature off; nothing breaks.

The only things genuinely requiring hardware are: exercising `CudaPool`, and
cross-checking against a live TensorRT-LLM instance. Both are explicitly deferred
and gated.

## 7. Phased Systems Lab plan

- **Phase 0 — Spec.** Write `SPEC.md`: block/pool model, retention semantics
  (priority, duration-TTL, revert-to-default, prioritized-LRU), prefix reuse,
  two lifecycles, invalid-input rules. (Reuses and extends MiniCache's spec.)
- **Phase 1 — Capabilities + invariants + reference model.** `DESIGN.md` + an
  independent oracle in Rust.
- **Phase 2 — Implement policy core** (`CpuPool` + retention engine + radix
  tree + eviction), std-only; CUDA behind a feature flag.
- **Phase 3 — Verify.** Unit tests (boundaries) + proptest (impl ≡ oracle) +
  repair workflow on a deliberately faulty fixture.
- **Phase 4 — Report + readiness decision.**

Each phase is reviewable; implementation does not begin until you approve this
plan and the scope in §3.

## 8. Open decisions for your review

1. **Scope:** confirm the §3 framing — build the *retention/eviction policy core*
   (hardware-agnostic), not a full GPU memory manager. (Recommended.)
2. **Target semantics:** mirror TensorRT-LLM's `KvCacheRetentionConfig` exactly
   (priority 0–100, default 35, duration_ms TTL, prioritized-LRU) — or a
   simplified subset first?
3. **Radix prefix reuse** in-scope now, or defer to a later pass?
4. **CUDA binding:** include the feature-gated `CudaPool` seam now (compile-only),
   or keep the first pass pure-CPU and add the seam later?
5. **Project home:** new `~/Projects/kvcache-labs/` vs. evolving `minicache-rust/`?

---

## 9. Frozen decisions (approved by user, 2026-10-15)

1. **Scope** → build the retention/eviction *policy core* (hardware-agnostic),
   not a GPU memory manager. ✅
2. **Semantics** → mirror TensorRT-LLM `KvCacheRetentionConfig` exactly:
   priority `0..=100` (default **35**), `duration_ms` TTL (priority reverts to
   default on expiry, block stays reusable), prioritized-LRU eviction. ✅
3. **Radix prefix reuse** → **deferred to Pass 2.** Pass 1 delivers the
   retention/eviction policy; prefix *lookup* (which blocks a token prefix maps
   to) is a distinct subsystem added next.
4. **CUDA byte-pool seam** → **deferred.** Pass 1 models block *metadata* only
   (priority/expiry/refcount/state), no tensor bytes; a `KvPool`-style device
   seam is a later integration layer.
5. **Project home** → **evolve `minicache-rust/`** in place (new modules
   alongside the existing TTL cache, which stays as the tested foundation).

## 10. Pass 1 implementation scope (retention/eviction policy core)

Deliverable: `BlockManager` — a fixed-size block pool with two lifecycles and
TensorRT-LLM-mirroring retention semantics.

- **Blocks** have three states: `Free`, `Active { refcount }`, and
  `Reusable { priority, expires_at, last_used }`.
- **Allocation** takes a free block, else evicts the lowest-*effective*-priority
  reusable block (LRU tie-break).
- **Release** decrements refcount; at zero the block becomes `Reusable` with the
  configured `(priority, duration_ms)` retention (`expires_at = now + duration`).
- **Reuse/acquire** promotes a `Reusable` block back to `Active` (refcount).
- **Duration TTL** — once `now >= expires_at`, the block's *effective* priority
  reverts to the default (35); it is **not** evicted, only deprioritized.
- **Invalidate** force-frees a block (idempotent).
- **Simulated clock + simulated pressure** (small pool) make eviction
  deterministic and testable with zero hardware.

Authoritative spec: `SPEC_RETENTION.md`. Capabilities/invariants/acceptance:
`DESIGN_RETENTION.md`. Verification: independent reference model + proptest
(impl ≡ oracle) + repair workflow.
