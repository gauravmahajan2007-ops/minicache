# PrefixIndex — Behavioral Specification (Radix-Trie Prefix Reuse)

Version 1.0 — Systems Lab Pass 2. Implements the prefix-reuse *lookup* structure
for KV-cache reuse (the piece Pass 1 deferred).

## 1. Purpose

`PrefixIndex` maps token sequences to block ids and answers *longest-prefix*
queries: given a request's token sequence, which cached blocks can be reused, and
how many leading tokens they cover. This is the lookup that, in a real engine,
feeds the `BlockManager.acquire` calls from Pass 1.

`block_key` is a deterministic, stable hash of a token span — the analog of
TensorRT-LLM's `makeKey` (a cross-language contract used by a KV-cache-aware
router to route a request to the worker holding its prefix).

## 2. Domain

- **Token** — a `u32` token id.
- **BlockId** — a `u32` (reused from `BlockManager`).
- A **mapping** is `(token_span, block_id)` where `token_span` is a non-empty
  sequence of tokens (the KV held by that block).

## 3. Semantics — the block-based contract

A stored block with span `P` **matches** a query `Q` iff `P` is a prefix of `Q`
(i.e. `Q[..len(P)] == P`). All matching blocks lie on a single root-to-node path
of the tree.

### 3.1 `insert(tokens, block)`
Records that `block` holds the KV for `tokens`. Overwriting an existing span
replaces its block (mirrors MiniCache's overwrite-reset). Inserting an empty
span is a no-op (no block is ever stored for zero tokens).

### 3.2 `longest_prefix(query) -> (matched, blocks)`
- `blocks` = every stored block whose span is a prefix of `query`, in increasing
  span-length (root-to-leaf) order.
- `matched` = the length of the **longest** such span (0 if none).

This is the exact contract; the trie is an efficient realization of it.

### 3.3 `get(tokens) -> Option<BlockId>`
Exact full-span lookup.

### 3.4 `remove(tokens) -> bool`
Removes the block at the exact span; returns whether it existed. Prunes now-empty
internal nodes (no block, no children) so `node_count()` shrinks and the trie
does not leak nodes.

### 3.5 `len() -> usize`, `is_empty()`, `node_count()`
`len` = number of stored blocks; `node_count` = number of trie nodes (1 = root
only; exposed for pruning verification).

## 4. block_key

`block_key(tokens: &[u32]) -> u64` is FNV-1a 64-bit over the little-endian bytes
of each token, in order (offset basis `0xcbf29ce484222325`, prime
`0x100000001b3`). It is **deterministic and stable** — the same span always
hashes to the same key, byte-for-byte, across processes and languages.

## 5. Invariants

- **I1 (longest-prefix correctness):** `longest_prefix` returns exactly the
  blocks whose full span is a prefix of the query, in root-to-leaf order.
- **I2 (exact vs prefix):** `get` returns `Some` only on an exact full-span match.
- **I3 (remove prunes):** after `remove`, `get` is `None`, `longest_prefix` no
  longer includes it, and empty nodes are pruned (`node_count` returns to 1 when
  the trie is empty).
- **I4 (overwrite):** inserting an existing span replaces the block without
  changing `len`.
- **I5 (empty):** `longest_prefix([]) == (0, [])`, `get([]) == None`,
  `insert([]) ` is a no-op, `remove([]) == false`.
- **I6 (block_key determinism):** identical spans hash to identical keys.

## 6. Reference model

An independent oracle (`ReferencePrefixIndex`, a `HashMap` of spans → blocks with
a naive prefix scan) encodes the same block-based contract. Property tests replay
random insert/get/remove/lookup sequences and require exact agreement.
