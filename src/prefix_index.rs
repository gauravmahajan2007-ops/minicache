//! `PrefixIndex` — a radix trie mapping token sequences to block ids, answering
//! longest-prefix queries for KV-cache reuse.
//!
//! The authoritative contract is the block-based semantic in `SPEC_PREFIX.md`;
//! this trie is an efficient realization of it. A plain (uncompressed) trie is
//! used: radix/Patricia compression is a space optimization that does not change
//! observable behavior.

use std::collections::HashMap;

/// A token id.
pub type Token = u32;

/// A trie node. `block` is the block whose KV span ends at this node; `children`
/// maps a token to a child node index.
struct Node {
    block: Option<u32>,
    children: HashMap<Token, usize>,
}

/// A prefix index over token sequences → block ids.
#[derive(Default)]
pub struct PrefixIndex {
    nodes: Vec<Node>,
    block_count: usize,
}

impl PrefixIndex {
    pub fn new() -> Self {
        Self {
            nodes: vec![Node {
                block: None,
                children: HashMap::new(),
            }],
            block_count: 0,
        }
    }

    /// Record that `block` holds the KV for `tokens`. Overwriting an existing
    /// span replaces its block. Inserting an empty span is a no-op.
    pub fn insert(&mut self, tokens: &[Token], block: u32) {
        if tokens.is_empty() {
            return;
        }
        let mut cur = 0usize;
        for &t in tokens {
            cur = match self.nodes[cur].children.get(&t) {
                Some(&child) => child,
                None => {
                    let idx = self.nodes.len();
                    self.nodes.push(Node {
                        block: None,
                        children: HashMap::new(),
                    });
                    self.nodes[cur].children.insert(t, idx);
                    idx
                }
            };
        }
        if self.nodes[cur].block.is_none() {
            self.block_count += 1;
        }
        self.nodes[cur].block = Some(block);
    }

    /// Exact full-span lookup.
    pub fn get(&self, tokens: &[Token]) -> Option<u32> {
        let mut cur = 0usize;
        for &t in tokens {
            cur = *self.nodes[cur].children.get(&t)?;
        }
        self.nodes[cur].block
    }

    /// Longest-prefix query. Returns `(matched_tokens, blocks)` where `blocks`
    /// are all stored blocks whose span is a prefix of `tokens`, in root-to-leaf
    /// order, and `matched_tokens` is the length of the longest such span.
    pub fn longest_prefix(&self, tokens: &[Token]) -> (usize, Vec<u32>) {
        let mut blocks = Vec::new();
        let mut matched = 0usize;
        let mut cur = 0usize;
        for (i, &t) in tokens.iter().enumerate() {
            match self.nodes[cur].children.get(&t) {
                Some(&child) => {
                    cur = child;
                    if let Some(b) = self.nodes[cur].block {
                        blocks.push(b);
                        matched = i + 1;
                    }
                }
                None => break,
            }
        }
        (matched, blocks)
    }

    /// Remove the block at the exact span; returns whether it existed. Prunes
    /// now-empty nodes (no block, no children) so the trie does not leak.
    pub fn remove(&mut self, tokens: &[Token]) -> bool {
        if tokens.is_empty() {
            return false;
        }
        // (parent_index, token_leading_to_child)
        let mut path: Vec<(usize, Token)> = Vec::with_capacity(tokens.len());
        let mut cur = 0usize;
        for &t in tokens {
            match self.nodes[cur].children.get(&t) {
                Some(&child) => {
                    path.push((cur, t));
                    cur = child;
                }
                None => return false,
            }
        }
        if self.nodes[cur].block.is_none() {
            return false;
        }
        self.nodes[cur].block = None;
        self.block_count -= 1;

        let mut node = cur;
        while node != 0 {
            let (parent, token) = path.pop().expect("path is non-empty");
            if self.nodes[node].block.is_none() && self.nodes[node].children.is_empty() {
                self.nodes[parent].children.remove(&token);
            } else {
                break;
            }
            node = parent;
        }
        true
    }

    /// Number of stored blocks.
    pub fn len(&self) -> usize {
        self.block_count
    }

    pub fn is_empty(&self) -> bool {
        self.block_count == 0
    }

    /// Number of trie nodes reachable from the root (1 = root only). Exposed for
    /// pruning verification. Pruned nodes are removed from the tree (unreachable);
    /// the arena may retain their slots as a known space tradeoff.
    pub fn node_count(&self) -> usize {
        let mut count = 0usize;
        let mut stack = vec![0usize];
        while let Some(idx) = stack.pop() {
            count += 1;
            for &child in self.nodes[idx].children.values() {
                stack.push(child);
            }
        }
        count
    }
}
