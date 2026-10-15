# PrefixIndex (Pass 2) — End-to-End Report

Systems Lab Pass 2: the prefix-reuse lookup structure (radix trie + block-key
hashing) for KV-cache reuse. Date: 2026-10-15 · rustc/cargo 1.99.0, proptest 1.12.0.

---

## 1. What was built

- **`PrefixIndex`** (`src/prefix_index.rs`) — a radix trie mapping token
  sequences to block ids, answering longest-prefix queries with a precise,
  block-based contract (SPEC_PREFIX.md §3): return every stored block whose span
  is a prefix of the query, in root-to-leaf order, plus the matched token count.
  Supports `insert` (overwrite-replaces), `get`, `remove` (with node pruning),
  `len`, and `node_count` (reachable-node count for pruning verification).
- **`block_key`** (`src/block_key.rs`) — FNV-1a 64-bit over little-endian token
  bytes; a deterministic, stable hash (the analog of TensorRT-LLM's `makeKey`).
  Locked by known-vector tests.

This is the lookup that feeds `BlockManager.acquire` from Pass 1; together they
cover both halves of KV-cache reuse (find the prefix → acquire its blocks).

## 2. Specification & design

`SPEC_PREFIX.md` (block-based contract, invariants I1–I6) and `DESIGN_PREFIX.md`
(capabilities, invariants, acceptance criteria).

## 3. Reference model

`tests/common/prefix_ref.rs` — an independent oracle (`ReferencePrefixIndex`, a
`HashMap` of spans → blocks with a naive linear prefix scan) plus a deliberately
faulty fixture (`FaultyPrefixIndex`).

## 4. Tests actually executed

**`cargo test` — 49 passed, 0 failed; `cargo clippy --all-targets -- -D warnings`
clean; `cargo fmt --check` clean.**

- `tests/prefix_unit.rs` (11) — I1–I6: full-chain longest-prefix, divergence,
  pruning (node_count back to 1), shared-node preservation, overwrite, empty
  handling, block_key determinism + known vectors.
- `tests/prefix_properties.rs` (1) — proptest: impl ≡ oracle over generated
  insert/get/remove/lookup sequences (500 cases), full agreement on
  longest-prefix, get, and len after every step.
- `tests/prefix_repair.rs` (3) — permanent chain regression guard, faulty-fixture
  defect documentation, committed-counterexample check.
- Existing Pass 1 + TTL suites still green (BlockManager 14 unit + retention
  property/repair; TTL cache 11 unit + property/repair).

## 5. Generated-test results (proptest)

```
PrefixIndex:   impl_agrees_with_reference   with_cases(500)   PASS
BlockManager:  impl_agrees_with_reference   with_cases(500)   PASS
TTL cache:     impl_agrees_with_reference   with_cases(300)   PASS
TTL cache:     invalid_ttl_agreement        with_cases(200)   PASS
```

1500 generated cases total, 0 failures.

## 6. Repair workflow — counterexample and repair evidence

`FaultyPrefixIndex` injects one defect: `longest_prefix` returns only the deepest
matching block, dropping shallower blocks in the chain (violates I1).

**Detection (authentic red, `evidence/repair-prefix-red-output.txt`):**

```
---- longest_prefix_against_faulty_fixture_detects_defect stdout ----
assertion `left == right` failed
  left: (2, [20])        # dropped the shallower block
 right: (2, [10, 20])    # full chain expected
test result: FAILED. 0 passed; 1 failed  (exit 101)
```

**Counterexample** (`counterexamples/prefix-chain-drop.json`): minimal sequence —
insert [1]→10, insert [1,2]→20, longest_prefix([1,2,3]) — observed `(2, [20])`,
expected `(2, [10, 20])`.

**Fix:** collect every matching block on the path (the shipped `PrefixIndex` does
this). The transient red test was removed and the full suite reran green
(**49 passed**), with `longest_prefix_chain_regression` locking the boundary.

## 7. Lessons captured (this run)

- An arena trie that "prunes" by removing edges still holds orphaned slots; a
  `node_count()` based on `Vec::len()` is wrong — count *reachable* nodes instead.
- FNV-1a known vectors were computed with a tool (not guessed), then locked as
  constants to catch any future algorithm drift.

## 8. Readiness decision

**READY (Pass 2).** Every acceptance criterion in DESIGN_PREFIX.md §3 is met.

Combined status: TTL cache (foundation) + BlockManager (Pass 1) + PrefixIndex
(Pass 2) — **49 tests, 1500 property cases, all green**, with an independent
oracle per subsystem and a demonstrated repair workflow for each.
