//! Independent reference model (oracle) and faulty fixture for `PrefixIndex`.
//!
//! `ReferencePrefixIndex` is a naive `HashMap` of spans → blocks with a linear
//! prefix scan — written directly from `SPEC_PREFIX.md`, with no reference to the
//! trie implementation, so agreement is meaningful.
//!
//! `FaultyPrefixIndex` is the same model with ONE injected defect: it returns
//! only the deepest matching block, dropping shallower matches.

use std::collections::HashMap;

/// The oracle: naive span → block map with a linear longest-prefix scan.
pub struct ReferencePrefixIndex {
    map: HashMap<Vec<u32>, u32>,
}

impl ReferencePrefixIndex {
    pub fn new() -> Self {
        Self {
            map: HashMap::new(),
        }
    }

    pub fn insert(&mut self, tokens: &[u32], block: u32) {
        if !tokens.is_empty() {
            self.map.insert(tokens.to_vec(), block);
        }
    }

    pub fn get(&self, tokens: &[u32]) -> Option<u32> {
        self.map.get(tokens).copied()
    }

    pub fn remove(&mut self, tokens: &[u32]) -> bool {
        self.map.remove(tokens).is_some()
    }

    pub fn len(&self) -> usize {
        self.map.len()
    }

    pub fn longest_prefix(&self, tokens: &[u32]) -> (usize, Vec<u32>) {
        let mut blocks = Vec::new();
        let mut matched = 0usize;
        for i in 1..=tokens.len() {
            if let Some(&b) = self.map.get(&tokens[..i]) {
                blocks.push(b);
                matched = i;
            }
        }
        (matched, blocks)
    }
}

/// Deliberately faulty fixture: identical to the oracle except
/// `longest_prefix` returns only the deepest matching block, dropping the
/// shallower blocks in the chain (violates SPEC_PREFIX.md §3.2 / invariant I1).
pub struct FaultyPrefixIndex {
    map: HashMap<Vec<u32>, u32>,
}

impl FaultyPrefixIndex {
    pub fn new() -> Self {
        Self {
            map: HashMap::new(),
        }
    }

    pub fn insert(&mut self, tokens: &[u32], block: u32) {
        if !tokens.is_empty() {
            self.map.insert(tokens.to_vec(), block);
        }
    }

    pub fn longest_prefix(&self, tokens: &[u32]) -> (usize, Vec<u32>) {
        // BUG: keeps only the deepest match, dropping shallower blocks
        let mut deepest: Option<(usize, u32)> = None;
        for i in 1..=tokens.len() {
            if let Some(&b) = self.map.get(&tokens[..i]) {
                deepest = Some((i, b));
            }
        }
        match deepest {
            Some((len, b)) => (len, vec![b]),
            None => (0, vec![]),
        }
    }
}
