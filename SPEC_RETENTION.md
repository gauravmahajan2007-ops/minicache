# MiniCache → BlockManager — Behavioral Specification (Retention/Eviction Core)

Version 1.0 — Systems Lab Pass 1. Mirrors TensorRT-LLM `KvCacheRetentionConfig`.

## 1. Purpose

`BlockManager` is the retention/eviction **policy core** of a KV cache: it decides
which of a fixed pool of blocks is free, active (in-flight), or reusable, and —
under memory pressure — which reusable block to evict. It manages block
**metadata** (priority, expiry, refcount, recency), not tensor bytes.

It evolves MiniCache's tested semantics (TTL expiry, overwrite-reset, idempotent
delete) into NVIDIA's retention model: **priority + duration-TTL + prioritized-LRU
eviction + two lifecycles**.

## 2. Domain

- **Block** — the unit of allocation, identified by `BlockId` (u32). A pool is a
  fixed number of blocks (0-indexed).
- **Priority** — `u8` in `0..=100`; **100 = most important**. **Default = 35**.
- **Duration** — `Option<f64>` milliseconds. `Some(d)` = the priority applies for
  `d` ms from first-reusable, then reverts to default. `None` = priority never
  reverts.
- **Time** — `f64`, supplied by a simulated clock (monotonic).
- **Recency** — a monotonic generation counter (u64) for LRU tie-breaking; every
  transition into `Reusable` stamps a strictly increasing `last_used`.

A block is in exactly one of three states:

```
Free                       — available for allocation
Active { refcount }        — in use by >= 1 in-flight sequence
Reusable { priority,       — freed but kept for reuse
           expires_at,     — absolute time the priority reverts (None = never)
           last_used }     — LRU recency stamp
```

## 3. Effective priority (the core rule)

A `Reusable` block stores a **configured** priority and an optional `expires_at`.
Its **effective** priority — the value used for eviction ordering — is:

```
effective(block) =
    if expires_at == Some(t) and now >= t   → DEFAULT (35)
    else                                     → configured priority
```

A block whose priority has reverted is **not** evicted for that reason; it simply
deprioritizes (stays reusable until memory pressure forces eviction).

## 4. Operations

### 4.1 `allocate() -> Option<BlockId>`
1. If a free block exists, take it (lowest `BlockId` first).
2. Else, evict the reusable block with the lowest **effective** priority; among
   equal priority, the **least recently used** (`last_used` smallest). Free it and
   use it.
3. Mark the block `Active { refcount: 1 }`, return its id.
4. If no free block and no reusable block (all active), return `None` (out of
   memory — active blocks are never evicted).

### 4.2 `release(id, retention) -> Result<(), Error>`
Valid only on an `Active` block. Decrements `refcount`; when it reaches 0 the
block transitions to `Reusable` with:
- `priority = retention.priority`
- `expires_at = retention.duration_ms.map(|d| now + d)`
- `last_used = next_generation()`

Releasing a non-`Active` block is an error.

### 4.3 `acquire(id) -> bool`
- `Reusable` → `Active { refcount: 1 }` (promotion; a "reuse hit"). Returns `true`.
- `Active` → `refcount += 1` (shared prefix). Returns `false`.
- `Free` → no-op, returns `false`.

### 4.4 `invalidate(id)`
Force-frees the block regardless of state (free, active, or reusable). Idempotent:
invalidating an already-`Free` block is a no-op. Callers must not invalidate a
block still referenced by an in-flight sequence.

### 4.5 `eviction_candidate() -> Option<BlockId>`
Returns the reusable block that `allocate` *would* evict next (lowest effective
priority, LRU tie-break), without mutating. `None` if no reusable block.

### 4.6 Queries
`is_free / is_active / is_reusable / effective_priority / refcount /
num_free / num_active / num_reusable`.

## 5. Invalid retention input

`Retention::new(priority, duration_ms)` rejects:
- `priority > 100` → `InvalidPriority`
- `duration_ms` finite and `<= 0`, or `NaN`/`±inf` → `InvalidDuration`

(`duration_ms = None` is always valid; priority `0` is valid — it means
"evict first".)

## 6. Invariants (must always hold)

- **I1 (default priority):** the default priority is 35.
- **I2 (duration revert):** a `Reusable` block with `expires_at = Some(t)` has
  effective priority = configured while `now < t`, and = 35 once `now >= t`.
- **I3 (prioritized LRU):** eviction always selects the reusable block with the
  lowest effective priority; ties broken by smallest `last_used`.
- **I4 (never evict active):** `Active` blocks (refcount ≥ 1) are never evicted;
  `allocate` returns `None` when no free or reusable block exists.
- **I5 (release → reusable):** refcount 0 ⇒ `Reusable`, never `Free` (a freed
  block is kept for reuse, not returned to the free pool).
- **I6 (reuse resets retention):** a `Reusable` block promoted to `Active` and
  later released again gets fresh `(priority, expires_at, last_used)` from the
  new release.
- **I7 (invalidate idempotent):** `invalidate` on `Free` is a no-op; after
  invalidate, the block is `Free`.
- **I8 (duration None = no revert):** `expires_at = None` ⇒ effective priority is
  always the configured priority.
- **I9 (monotonic recency):** `last_used` is strictly increasing across
  transitions into `Reusable`.

## 7. Reference model

An independent oracle (`ReferenceBlockManager`) encodes the same semantics,
written from this spec with no reference to the implementation. Property tests
replay identical operation sequences and require exact agreement on block state,
effective priority, refcount, eviction candidate, and allocation result.
