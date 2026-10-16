//! Independent reference model (oracle) and faulty fixtures for the BlockManager.
//!
//! `ReferenceBlockManager` is written directly from `SPEC_RETENTION.md` with no
//! reference to the implementation. To keep it genuinely independent it does
//! NOT maintain a free list: the free set is *derived* by scanning the state
//! vector, so it encodes "lowest `BlockId` first" (§4.1) from the spec rather
//! than copying the implementation's data structure. If the implementation's
//! free list ever diverged from "lowest id first" (as it once did, FIFO), this
//! oracle would disagree and the property test would fail.
//!
//! `FaultyBlockManager` and the `SwitchFaulty` fixtures each carry ONE injected
//! defect (a guard removed), for the switch-off verification workflow.

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
///
/// Deliberately different from the implementation in representation: no
/// free-list data structure, the free set is derived by a linear scan, and
/// eviction is a linear scan over `(effective_priority, last_used, id)`.
pub struct ReferenceBlockManager {
    clock: SimulatedClock,
    generation: u64,
    blocks: Vec<RefBlockState>,
}

impl ReferenceBlockManager {
    pub fn new(num_blocks: usize, clock: SimulatedClock) -> Self {
        Self {
            clock,
            generation: 0,
            blocks: vec![RefBlockState::Free; num_blocks],
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

    /// Lowest free `BlockId`, or `None` — derived by scan, not a maintained list.
    fn lowest_free(&self) -> Option<BlockId> {
        self.blocks
            .iter()
            .enumerate()
            .find(|(_, s)| matches!(s, RefBlockState::Free))
            .map(|(i, _)| i as BlockId)
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
        if let Some(id) = self.lowest_free() {
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
        self.blocks
            .iter()
            .filter(|s| matches!(s, RefBlockState::Free))
            .count()
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

/// Deliberately faulty fixture (I2): identical to the reference model except the
/// duration-revert comparison uses `>` instead of `>=`, so a block's priority
/// fails to revert at *exactly* its expiry time (violates SPEC_RETENTION.md §3).
pub struct FaultyBlockManager {
    clock: SimulatedClock,
    generation: u64,
    blocks: Vec<RefBlockState>,
}

impl FaultyBlockManager {
    pub fn new(num_blocks: usize, clock: SimulatedClock) -> Self {
        Self {
            clock,
            generation: 0,
            blocks: vec![RefBlockState::Free; num_blocks],
        }
    }

    pub fn clock_mut(&mut self) -> &mut SimulatedClock {
        &mut self.clock
    }

    pub fn allocate(&mut self) -> Option<BlockId> {
        let id = self
            .blocks
            .iter()
            .enumerate()
            .find(|(_, s)| matches!(s, RefBlockState::Free))
            .map(|(i, _)| i as BlockId)?;
        self.blocks[id as usize] = RefBlockState::Active(1);
        Some(id)
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

/// One switch-off defect among the invariants, for the switch fixtures.
#[derive(Debug, Clone, Copy)]
pub enum Defect {
    /// I3: LRU tie-break inverted (evicts most-recently-used among equal priority).
    LruTiebreak,
    /// I4: evicts an Active block when no free/reusable block exists.
    EvictsActive,
    /// I6: reuse does not reset retention (stale expiry survives).
    ReuseKeepsRetention,
    /// I7: invalidate is not idempotent (re-freeing corrupts the free count).
    Invalidate,
}

/// A faulty fixture that exhibits exactly one of the switch-off defects. Used
/// only by the switch tests in `tests/retention_switches.rs`.
pub struct SwitchFaulty {
    clock: SimulatedClock,
    generation: u64,
    blocks: Vec<RefBlockState>,
    defect: Defect,
    free_count: usize, // used by the Invalidate defect to hold a corrupt count
    remembered_expiry: Option<f64>, // used by ReuseKeepsRetention to hold a stale expiry
}

impl SwitchFaulty {
    pub fn new(num_blocks: usize, defect: Defect) -> Self {
        Self {
            clock: SimulatedClock::new(),
            generation: 0,
            blocks: vec![RefBlockState::Free; num_blocks],
            defect,
            free_count: num_blocks,
            remembered_expiry: None,
        }
    }

    pub fn clock_mut(&mut self) -> &mut SimulatedClock {
        &mut self.clock
    }

    pub fn allocate(&mut self) -> Option<BlockId> {
        let free = self
            .blocks
            .iter()
            .enumerate()
            .find(|(_, s)| matches!(s, RefBlockState::Free))
            .map(|(i, _)| i as BlockId);
        match free {
            Some(id) => {
                self.blocks[id as usize] = RefBlockState::Active(1);
                self.free_count -= 1;
                Some(id)
            }
            None => match self.defect {
                Defect::EvictsActive => {
                    // BUG (I4): evict an Active block instead of returning None.
                    let id = self
                        .blocks
                        .iter()
                        .enumerate()
                        .find(|(_, s)| matches!(s, RefBlockState::Active(_)))
                        .map(|(i, _)| i as BlockId)?;
                    self.blocks[id as usize] = RefBlockState::Active(1);
                    Some(id)
                }
                _ => None,
            },
        }
    }

    pub fn release(&mut self, id: BlockId, r: Retention) -> Result<(), ()> {
        match self.blocks[id as usize] {
            RefBlockState::Active(rc) => {
                if rc == 1 {
                    let now = self.clock.now();
                    // I6 defect: reuse keeps the stale expiry instead of applying
                    // fresh retention (the FIRST release's expiry is reused).
                    let expires_at = match self.defect {
                        Defect::ReuseKeepsRetention if self.remembered_expiry.is_some() => {
                            self.remembered_expiry
                        }
                        _ => r.duration_ms.map(|d| now + d),
                    };
                    if expires_at.is_some() {
                        self.remembered_expiry = expires_at;
                    }
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
        match self.defect {
            Defect::Invalidate => {
                // BUG (I7): not idempotent — re-freeing a Free block corrupts count.
                self.blocks[id as usize] = RefBlockState::Free;
                self.free_count += 1;
            }
            _ => {
                if !matches!(self.blocks[id as usize], RefBlockState::Free) {
                    self.blocks[id as usize] = RefBlockState::Free;
                    self.free_count += 1;
                }
            }
        }
    }

    /// LRU tie-break with the I3 defect: among equal effective priority, pick
    /// the *most* recently used (largest `last_used`) instead of least.
    pub fn eviction_candidate(&self) -> Option<BlockId> {
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
                let better = match self.defect {
                    Defect::LruTiebreak => {
                        best.is_none_or(|(b0, b1, _)| key.0 < b0 || (key.0 == b0 && key.1 > b1))
                    }
                    _ => best.is_none_or(|(b0, b1, b2)| key < (b0, b1, b2)),
                };
                if better {
                    best = Some(key);
                }
            }
        }
        best.map(|(_, _, id)| id)
    }

    pub fn effective_priority(&self, id: BlockId) -> Option<u8> {
        match self.blocks[id as usize] {
            RefBlockState::Reusable {
                priority,
                expires_at: Some(e),
                ..
            } => {
                if self.clock.now() >= e {
                    Some(DEFAULT_PRIORITY)
                } else {
                    Some(priority)
                }
            }
            RefBlockState::Reusable {
                priority,
                expires_at: None,
                ..
            } => Some(priority),
            _ => None,
        }
    }

    pub fn num_free(&self) -> usize {
        self.free_count
    }

    pub fn is_free(&self, id: BlockId) -> bool {
        matches!(self.blocks[id as usize], RefBlockState::Free)
    }
}
