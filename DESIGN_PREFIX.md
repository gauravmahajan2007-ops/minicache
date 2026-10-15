# PrefixIndex — Capabilities, Invariants, Acceptance Criteria

## 1. Capabilities and dependencies

| Capability | Depends on | Notes |
|---|---|---|
| C1. Trie arena (nodes + token edges) | std (`HashMap`) | root = node 0 |
| C2. Insert/get/remove block refs | C1 | overwrite replaces; remove prunes |
| C3. Longest-prefix lookup | C1 | block-based contract (SPEC_PREFIX §3) |
| C4. block_key (FNV-1a 64) | std | deterministic, stable |
| C5. Reference model (oracle) | std | independent `HashMap` + naive scan |
| C6. Property-based harness | proptest (dev) | replays op sequences on C2/C3 vs C5 |

Dependency graph: `C1 → C2, C3`; `{C2,C3} → PrefixIndex`; `{PrefixIndex, C5} → C6`;
`C4` independent.

## 2. Invariants (see SPEC_PREFIX.md §5)

I1 longest-prefix correctness · I2 exact vs prefix · I3 remove prunes ·
I4 overwrite · I5 empty handling · I6 block_key determinism.

## 3. Acceptance criteria (testable)

1. All unit tests pass — I1–I6, pruning, overwrite, empty handling, and block_key
   determinism + known vectors.
2. All property tests pass — implementation and reference model agree on every
   generated insert/get/remove/lookup sequence (longest_prefix result, get, len).
3. Repair workflow demonstrated with real red output, a saved counterexample, a
   fix, and a green rerun.
4. Readiness decision recorded in `REPORT_PREFIX.md`.

## 4. What "done" does NOT mean

A trie that passes simple unit tests can still drop intermediate blocks on a
multi-block chain (the exact bug class the property test hunts). The property
agreement test and the longest-prefix regression test are the real gates.
