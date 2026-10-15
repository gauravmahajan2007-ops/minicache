//! `BlockManager` — the retention/eviction policy core of a KV cache.
//!
//! Manages a fixed pool of blocks across two lifecycles (active vs reusable)
//! with TensorRT-LLM-mirroring retention semantics: priority, duration-TTL
//! (priority reverts to default on expiry), and prioritized-LRU eviction.
//!
//! See `SPEC_RETENTION.md` for the authoritative contract.

use std::collections::VecDeque;

use crate::retention::{Retention, RetentionError, DEFAULT_PRIORITY};
use crate::SimulatedClock;

/// Identifies a block within the pool.
pub type BlockId = u32;

/// The lifecycle state of a block. Public so tests (and the reference model)
/// can inspect and compare exact state.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BlockState {
    Free,
    Active {
        refcount: u32,
    },
    Reusable {
        priority: u8,
        expires_at: Option<f64>,
        last_used: u64,
    },
}

/// A fixed-size block pool with retention/eviction policy.
pub struct BlockManager {
    clock: SimulatedClock,
    generation: u64,
    blocks: Vec<BlockState>,
    free: VecDeque<BlockId>,
}

impl BlockManager {
    pub fn new(num_blocks: usize, clock: SimulatedClock) -> Self {
        let blocks = vec![BlockState::Free; num_blocks];
        let free = (0..num_blocks as BlockId).collect();
        Self {
            clock,
            generation: 0,
            blocks,
            free,
        }
    }

    pub fn with_capacity(num_blocks: usize) -> Self {
        Self::new(num_blocks, SimulatedClock::new())
    }

    pub fn clock(&self) -> &SimulatedClock {
        &self.clock
    }

    pub fn clock_mut(&mut self) -> &mut SimulatedClock {
        &mut self.clock
    }

    /// Effective priority of a reusable block: configured priority unless the
    /// duration has elapsed (then the default).
    fn effective_priority_of(state: &BlockState, now: f64) -> u8 {
        match state {
            BlockState::Reusable {
                priority,
                expires_at: Some(exp),
                ..
            } => {
                if now >= *exp {
                    DEFAULT_PRIORITY
                } else {
                    *priority
                }
            }
            BlockState::Reusable {
                priority,
                expires_at: None,
                ..
            } => *priority,
            _ => DEFAULT_PRIORITY,
        }
    }

    pub fn allocate(&mut self) -> Option<BlockId> {
        if let Some(id) = self.free.pop_front() {
            self.blocks[id as usize] = BlockState::Active { refcount: 1 };
            return Some(id);
        }
        let victim = self.eviction_candidate()?;
        self.blocks[victim as usize] = BlockState::Active { refcount: 1 };
        Some(victim)
    }

    pub fn eviction_candidate(&self) -> Option<BlockId> {
        let now = self.clock.now();
        self.blocks
            .iter()
            .enumerate()
            .filter_map(|(i, s)| match s {
                BlockState::Reusable { last_used, .. } => {
                    let eff = Self::effective_priority_of(s, now);
                    Some((eff, *last_used, i as BlockId))
                }
                _ => None,
            })
            .min_by_key(|(eff, last_used, _)| (*eff, *last_used))
            .map(|(_, _, id)| id)
    }

    pub fn release(&mut self, id: BlockId, retention: Retention) -> Result<(), RetentionError> {
        let state = self.blocks[id as usize];
        match state {
            BlockState::Active { refcount } => {
                if refcount == 1 {
                    let now = self.clock.now();
                    let expires_at = retention.duration_ms.map(|d| now + d);
                    self.generation += 1;
                    self.blocks[id as usize] = BlockState::Reusable {
                        priority: retention.priority,
                        expires_at,
                        last_used: self.generation,
                    };
                } else {
                    self.blocks[id as usize] = BlockState::Active {
                        refcount: refcount - 1,
                    };
                }
                Ok(())
            }
            _ => Err(RetentionError::NotActive(id)),
        }
    }

    pub fn acquire(&mut self, id: BlockId) -> bool {
        let state = self.blocks[id as usize];
        match state {
            BlockState::Active { refcount } => {
                self.blocks[id as usize] = BlockState::Active {
                    refcount: refcount + 1,
                };
                false
            }
            BlockState::Reusable { .. } => {
                self.blocks[id as usize] = BlockState::Active { refcount: 1 };
                true
            }
            BlockState::Free => false,
        }
    }

    pub fn invalidate(&mut self, id: BlockId) {
        if self.blocks[id as usize] != BlockState::Free {
            self.blocks[id as usize] = BlockState::Free;
            self.free.push_back(id);
        }
    }

    pub fn state(&self, id: BlockId) -> BlockState {
        self.blocks[id as usize]
    }

    pub fn is_free(&self, id: BlockId) -> bool {
        matches!(self.blocks[id as usize], BlockState::Free)
    }

    pub fn is_active(&self, id: BlockId) -> bool {
        matches!(self.blocks[id as usize], BlockState::Active { .. })
    }

    pub fn is_reusable(&self, id: BlockId) -> bool {
        matches!(self.blocks[id as usize], BlockState::Reusable { .. })
    }

    /// Effective priority of a reusable block; `None` for free/active blocks.
    pub fn effective_priority(&self, id: BlockId) -> Option<u8> {
        match self.blocks[id as usize] {
            BlockState::Reusable { .. } => Some(Self::effective_priority_of(
                &self.blocks[id as usize],
                self.clock.now(),
            )),
            _ => None,
        }
    }

    /// Absolute expiry time of a reusable block's priority; `None` for
    /// free/active blocks, or for a reusable block with no duration.
    pub fn expires_at(&self, id: BlockId) -> Option<Option<f64>> {
        match self.blocks[id as usize] {
            BlockState::Reusable { expires_at, .. } => Some(expires_at),
            _ => None,
        }
    }

    /// Refcount of an active block; `None` otherwise.
    pub fn refcount(&self, id: BlockId) -> Option<u32> {
        match self.blocks[id as usize] {
            BlockState::Active { refcount } => Some(refcount),
            _ => None,
        }
    }

    pub fn num_free(&self) -> usize {
        self.free.len()
    }

    pub fn num_active(&self) -> usize {
        self.blocks
            .iter()
            .filter(|s| matches!(s, BlockState::Active { .. }))
            .count()
    }

    pub fn num_reusable(&self) -> usize {
        self.blocks
            .iter()
            .filter(|s| matches!(s, BlockState::Reusable { .. }))
            .count()
    }
}
