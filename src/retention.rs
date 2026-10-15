//! Retention policy types for the KV-cache block manager.
//!
//! Mirrors TensorRT-LLM's `KvCacheRetentionConfig`: a reusable block carries a
//! priority (0..=100, default 35) and an optional duration over which that
//! priority applies before reverting to the default.

use crate::BlockId;

/// The default retention priority (TensorRT-LLM uses 35).
pub const DEFAULT_PRIORITY: u8 = 35;
/// The maximum valid priority.
pub const MAX_PRIORITY: u8 = 100;

/// Errors returned by retention construction and block-manager operations.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RetentionError {
    /// Priority exceeded `MAX_PRIORITY`.
    InvalidPriority(u8),
    /// Duration was not a finite, strictly positive number.
    InvalidDuration(f64),
    /// An operation required the block to be `Active`, but it was not.
    NotActive(BlockId),
}

/// The retention policy assigned to a block when it becomes reusable.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Retention {
    pub priority: u8,
    pub duration_ms: Option<f64>,
}

impl Retention {
    /// Construct a `Retention`, validating priority (<= 100) and duration
    /// (finite and > 0, or `None`).
    pub fn new(priority: u8, duration_ms: Option<f64>) -> Result<Self, RetentionError> {
        if priority > MAX_PRIORITY {
            return Err(RetentionError::InvalidPriority(priority));
        }
        if let Some(d) = duration_ms {
            if !d.is_finite() || d <= 0.0 {
                return Err(RetentionError::InvalidDuration(d));
            }
        }
        Ok(Self {
            priority,
            duration_ms,
        })
    }
}

impl Default for Retention {
    fn default() -> Self {
        Self {
            priority: DEFAULT_PRIORITY,
            duration_ms: None,
        }
    }
}
