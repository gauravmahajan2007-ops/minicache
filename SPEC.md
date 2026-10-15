# MiniCache — Behavioral Specification (Rust realization)

Version 1.0 — first end-to-end test of Hermes Systems Lab, Rust edition.

## 1. Purpose

MiniCache is a deliberately minimal in-memory TTL key-value cache. It exercises
the Systems Lab methodology (spec → capabilities → invariants → reference model
→ implementation → property verification → repair) on a problem small enough to
reason about precisely, yet non-trivial enough that off-by-one and
state-management defects are realistic.

**Language:** Rust (cargo + proptest). Memory ownership, borrow checking, and the
test discipline are part of the point — a Systems Lab does not hide them behind a
garbage collector.

**Explicitly out of scope** (and *forbidden* in this deliverable):

- networking / RPC
- persistence to disk
- distributed replication / consensus
- external databases
- background reaper threads / timers
- any runtime dependency beyond the standard library (only `proptest`, test-time)

## 2. Domain

- **Key** — any `Eq + Hash` type (`K`).
- **Value** — any type (`V`).
- **TTL** — time-to-live, a *finite, strictly positive* `f64` (seconds).
- **Time** — an `f64` supplied by a *simulated clock*; never real wall time.

A stored entry is the pair `(value, expiry)` where `expiry` is an **absolute**
time computed at write: `expiry = now + ttl`.

### Language note on type-level distinctions

Two Python-era rules are enforced by Rust's type system rather than at runtime:

- **Missing vs stored value** is `Option<&V>`: `None` means absent/expired,
  `Some(&v)` means a live value. There is no ambiguity between "missing" and "a
  stored null", because a value of type `V` cannot *be* `None`.
- **Non-numeric / boolean TTL** cannot be expressed: `ttl` is typed `f64`, so the
  compiler rejects anything else. The remaining runtime checks are *finite* and
  *strictly positive*.

## 3. Operations

### 3.1 `put(key, value, ttl) -> Result<(), Error>`

Stores `value` under `key`, expiring at `now + ttl`.

- **Overwrite semantics (documented):** overwriting an existing key replaces
  *both* the value *and* the expiry. The new expiry is `now + ttl` measured from
  the time of the *new* put — not the maximum of old and new, not inherited.
  An overwrite *resets* the TTL.
- **Overwriting a stale (expired but not yet evicted) entry** behaves exactly like
  a fresh put.
- **Invalid TTL:** `ttl` must be finite (no `NaN`, no `±inf`) and strictly `> 0`.
  Any invalid TTL returns `Err(Error::InvalidTtl)` and leaves state unchanged.

### 3.2 `get(key) -> Option<&V>`

Returns `Some(&value)` while the entry is *live*; otherwise `None`.

- **Liveness boundary (the precise contract):** an entry is live iff
  `now < expiry`. At `now == expiry` the entry is **expired**, not live.
  Equivalent statement: *a key cannot be read at or after its expiry time.*
- A key never stored returns `None`.
- **Lazy eviction:** the first `get` that observes an entry expired removes it,
  so a subsequent `get` returns `None` and `size()` reflects the removal.

### 3.3 `delete(key)`

Removes the key if present.

- Deleting a missing key is a **safe, repeatable no-op** (idempotent).
- Deleting an expired-but-present entry also removes it.

### 3.4 `size() -> usize`

Returns the number of **live** entries (`now < expiry`). Does not mutate the
store; expired entries are removed lazily by `get`/`delete`.

## 4. Clock

`SimulatedClock` starts at `0.0` (configurable) and only moves forward.

- `now()` returns the current time.
- `advance(delta) -> Result<f64, Error>` adds a **finite, non-negative** `delta`.
  A negative or non-finite `delta` returns `Err(Error::InvalidAdvance)`.
- Time is **monotonically non-decreasing**.

## 5. Expiry boundary — worked example

```
t=0     put("k", 1, ttl=10)   → expiry = 10
t=5     get("k")                → Some(1)     (5 < 10)
t=9.99  get("k")                → Some(1)
t=10    get("k")                → None        (10 >= 10, expired)
t=10    size()                  → 0
t=10    delete("k")             → no-op
```

Overwrite:

```
t=0   put("k", 1, ttl=10)   → expiry 10
t=5   put("k", 2, ttl=10)   → expiry 15 (reset, NOT max/inherit)
t=12  get("k")                → Some(2)   (12 < 15)
```

## 6. Reference model

An independent oracle lives in `tests/common/mod.rs` (`ReferenceCache`,
`ReferenceClock`), written directly from this specification with no reference to
the implementation. Property-based tests replay identical operation sequences
against both and require exact agreement.
