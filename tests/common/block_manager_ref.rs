//! Independent reference model (oracle) and faulty fixture for the BlockManager.
//!
//! `ReferenceBlockManager` is written directly from `SPEC_RETENTION.md` with no
//! reference to the implementation — its state machine is intentionally naive so
//! agreement with `BlockManager` is meaningful.
//!
//! `FaultyBlockManager` is the same model with ONE injected defect (the
//! duration-revert comparison uses `>` instead of `>=`), for the repair workflow.

use std::collections::VecDeque;

use minicache::{BlockId, Retention, SimulatedClock, DEFAULT_PRIORITY};

/// Naive independent block state.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RefBlockState {
    Free,
    Active(u32),
    Reusable {
        priority: u8,
        expires_at: Option<f64>,
        last_used: u64,
    },
}

/// The oracle: an independent, naive realization of `SPEC_RETENTION.md`.
pub struct ReferenceBlockManager {
    clock: SimulatedClock,
    generation: u64,
    blocks: Vec<RefBlockState>,
    free: VecDeque<BlockId>,
}

impl ReferenceBlockManager {
    pub fn new(num_blocks: usize, clock: SimulatedClock) -> Self {
        Self {
            clock,
            generation: 0,
            blocks: vec![RefBlockState::Free; num_blocks],
            free: (0..num_blocks as BlockId).collect(),
        }
    }

    pub fn with_capacity(num_blocks: usize) -> Self {
        Self::new(num_blocks, SimulatedClock::new())
    }

    pub fn clock_mut(&mut self) -> &mut SimulatedClock {
        &mut self.clock
    }

    fn eff(&self, s: &RefBlockState) -> u8 {
        match s {
            RefBlockState::Reusable {
                priority,
                expires_at: Some(e),
                ..
            } => {
                if self.clock.now() >= *e {
                    DEFAULT_PRIORITY
                } else {
                    *priority
                }
            }
            RefBlockState::Reusable {
                priority,
                expires_at: None,
                ..
            } => *priority,
            _ => DEFAULT_PRIORITY,
        }
    }

    fn min_reusable(&self) -> Option<BlockId> {
        let now = self.clock.now();
        let mut best: Option<(u8, u64, BlockId)> = None;
        for (i, s) in self.blocks.iter().enumerate() {
            if let RefBlockState::Reusable {
                priority,
                expires_at,
                last_used,
            } = s
            {
                let eff = match expires_at {
                    Some(e) if now >= *e => DEFAULT_PRIORITY,
                    _ => *priority,
                };
                let key = (eff, *last_used, i as BlockId);
                if best.is_none_or(|(b0, b1, b2)| key < (b0, b1, b2)) {
                    best = Some(key);
                }
            }
        }
        best.map(|(_, _, id)| id)
    }

    pub fn allocate(&mut self) -> Option<BlockId> {
        if let Some(id) = self.free.pop_front() {
            self.blocks[id as usize] = RefBlockState::Active(1);
            return Some(id);
        }
        let id = self.min_reusable()?;
        self.blocks[id as usize] = RefBlockState::Active(1);
        Some(id)
    }

    pub fn eviction_candidate(&self) -> Option<BlockId> {
        self.min_reusable()
    }

    pub fn release(&mut self, id: BlockId, r: Retention) -> Result<(), ()> {
        match self.blocks[id as usize] {
            RefBlockState::Active(rc) => {
                if rc == 1 {
                    let now = self.clock.now();
                    let expires_at = r.duration_ms.map(|d| now + d);
                    self.generation += 1;
                    self.blocks[id as usize] = RefBlockState::Reusable {
                        priority: r.priority,
                        expires_at,
                        last_used: self.generation,
                    };
                } else {
                    self.blocks[id as usize] = RefBlockState::Active(rc - 1);
                }
                Ok(())
            }
            _ => Err(()),
        }
    }

    pub fn acquire(&mut self, id: BlockId) -> bool {
        match self.blocks[id as usize] {
            RefBlockState::Active(rc) => {
                self.blocks[id as usize] = RefBlockState::Active(rc + 1);
                false
            }
            RefBlockState::Reusable { .. } => {
                self.blocks[id as usize] = RefBlockState::Active(1);
                true
            }
            RefBlockState::Free => false,
        }
    }

    pub fn invalidate(&mut self, id: BlockId) {
        if !matches!(self.blocks[id as usize], RefBlockState::Free) {
            self.blocks[id as usize] = RefBlockState::Free;
            self.free.push_back(id);
        }
    }

    pub fn state(&self, id: BlockId) -> RefBlockState {
        self.blocks[id as usize]
    }

    pub fn is_free(&self, id: BlockId) -> bool {
        matches!(self.blocks[id as usize], RefBlockState::Free)
    }
    pub fn is_active(&self, id: BlockId) -> bool {
        matches!(self.blocks[id as usize], RefBlockState::Active(_))
    }
    pub fn is_reusable(&self, id: BlockId) -> bool {
        matches!(self.blocks[id as usize], RefBlockState::Reusable { .. })
    }

    pub fn effective_priority(&self, id: BlockId) -> Option<u8> {
        match self.blocks[id as usize] {
            RefBlockState::Reusable { .. } => {
                let s = self.blocks[id as usize];
                Some(self.eff(&s))
            }
            _ => None,
        }
    }

    pub fn expires_at(&self, id: BlockId) -> Option<Option<f64>> {
        match self.blocks[id as usize] {
            RefBlockState::Reusable { expires_at, .. } => Some(expires_at),
            _ => None,
        }
    }

    pub fn refcount(&self, id: BlockId) -> Option<u32> {
        match self.blocks[id as usize] {
            RefBlockState::Active(rc) => Some(rc),
            _ => None,
        }
    }

    pub fn num_free(&self) -> usize {
        self.free.len()
    }
    pub fn num_active(&self) -> usize {
        self.blocks
            .iter()
            .filter(|s| matches!(s, RefBlockState::Active(_)))
            .count()
    }
    pub fn num_reusable(&self) -> usize {
        self.blocks
            .iter()
            .filter(|s| matches!(s, RefBlockState::Reusable { .. }))
            .count()
    }
}

/// Deliberately faulty fixture: identical to the reference model except the
/// duration-revert comparison uses `>` instead of `>=`, so a block's priority
/// fails to revert at *exactly* its expiry time (violates SPEC_RETENTION.md §3).
pub struct FaultyBlockManager {
    clock: SimulatedClock,
    generation: u64,
    blocks: Vec<RefBlockState>,
    free: VecDeque<BlockId>,
}

impl FaultyBlockManager {
    pub fn new(num_blocks: usize, clock: SimulatedClock) -> Self {
        Self {
            clock,
            generation: 0,
            blocks: vec![RefBlockState::Free; num_blocks],
            free: (0..num_blocks as BlockId).collect(),
        }
    }

    pub fn clock_mut(&mut self) -> &mut SimulatedClock {
        &mut self.clock
    }

    pub fn allocate(&mut self) -> Option<BlockId> {
        if let Some(id) = self.free.pop_front() {
            self.blocks[id as usize] = RefBlockState::Active(1);
            return Some(id);
        }
        // BUG lives in effective-priority computation, exercised via release.
        None
    }

    pub fn release(&mut self, id: BlockId, r: Retention) -> Result<(), ()> {
        match self.blocks[id as usize] {
            RefBlockState::Active(rc) => {
                if rc == 1 {
                    let now = self.clock.now();
                    let expires_at = r.duration_ms.map(|d| now + d);
                    self.generation += 1;
                    self.blocks[id as usize] = RefBlockState::Reusable {
                        priority: r.priority,
                        expires_at,
                        last_used: self.generation,
                    };
                } else {
                    self.blocks[id as usize] = RefBlockState::Active(rc - 1);
                }
                Ok(())
            }
            _ => Err(()),
        }
    }

    pub fn effective_priority(&self, id: BlockId) -> Option<u8> {
        match self.blocks[id as usize] {
            RefBlockState::Reusable {
                priority,
                expires_at: Some(e),
                ..
            } => {
                // BUG: `>` should be `>=`
                Some(if self.clock.now() > e {
                    DEFAULT_PRIORITY
                } else {
                    priority
                })
            }
            RefBlockState::Reusable {
                priority,
                expires_at: None,
                ..
            } => Some(priority),
            _ => None,
        }
    }
}
