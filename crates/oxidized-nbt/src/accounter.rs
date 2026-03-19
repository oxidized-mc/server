//! Memory accounting for NBT parsing.
//!
//! Tracks cumulative byte usage and nesting depth while reading NBT data,
//! preventing denial-of-service via pathologically large or deeply nested
//! payloads. The quotas and depth limits match the vanilla Minecraft server.

use crate::error::{DEFAULT_QUOTA, MAX_DEPTH, NbtError, UNCOMPRESSED_QUOTA};

/// Tracks cumulative memory usage and nesting depth during NBT parsing.
///
/// Create one per parse operation and pass it through every recursive call.
/// The accounter ensures that malicious or malformed payloads cannot exhaust
/// memory or blow the stack.
///
/// # Examples
///
/// ```
/// use oxidized_nbt::NbtAccounter;
///
/// // Use unlimited() for trusted data where quotas are not needed.
/// let mut acc = NbtAccounter::unlimited();
/// acc.account_bytes(1024).unwrap();
/// assert_eq!(acc.usage(), 1024);
///
/// // Use default_quota() for network data (2 MiB limit).
/// let acc = NbtAccounter::default_quota();
/// ```
pub struct NbtAccounter {
    usage: usize,
    quota: usize,
    depth: usize,
    max_depth: usize,
}

impl NbtAccounter {
    /// Creates an accounter with the given byte quota and default max depth.
    pub fn new(quota: usize) -> Self {
        Self {
            usage: 0,
            quota,
            depth: 0,
            max_depth: MAX_DEPTH,
        }
    }

    /// Creates an accounter with the default network quota (2 MiB).
    pub fn default_quota() -> Self {
        Self::new(DEFAULT_QUOTA)
    }

    /// Creates an accounter with the uncompressed disk quota (100 MiB).
    pub fn uncompressed_quota() -> Self {
        Self::new(UNCOMPRESSED_QUOTA)
    }

    /// Creates an accounter that effectively imposes no byte limit.
    pub fn unlimited() -> Self {
        Self::new(usize::MAX)
    }

    /// Adds `bytes` to the cumulative usage counter.
    ///
    /// # Errors
    ///
    /// Returns [`NbtError::SizeLimit`] if the new usage exceeds the quota.
    pub fn account_bytes(&mut self, bytes: usize) -> Result<(), NbtError> {
        self.usage = self.usage.saturating_add(bytes);
        if self.usage > self.quota {
            return Err(NbtError::SizeLimit {
                used: self.usage,
                quota: self.quota,
            });
        }
        Ok(())
    }

    /// Increments the nesting depth by one.
    ///
    /// # Errors
    ///
    /// Returns [`NbtError::DepthLimit`] if the new depth exceeds the maximum.
    pub fn push_depth(&mut self) -> Result<(), NbtError> {
        self.depth += 1;
        if self.depth > self.max_depth {
            return Err(NbtError::DepthLimit {
                depth: self.depth,
                max: self.max_depth,
            });
        }
        Ok(())
    }

    /// Decrements the nesting depth by one.
    ///
    /// # Panics
    ///
    /// Debug-asserts that depth is greater than zero.
    pub fn pop_depth(&mut self) {
        debug_assert!(self.depth > 0, "NbtAccounter depth underflow");
        self.depth = self.depth.saturating_sub(1);
    }

    /// Returns the cumulative byte usage so far.
    pub fn usage(&self) -> usize {
        self.usage
    }

    /// Returns the current nesting depth.
    pub fn depth(&self) -> usize {
        self.depth
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;

    #[test]
    fn test_byte_accounting() {
        let mut acc = NbtAccounter::new(100);
        acc.account_bytes(30).unwrap();
        assert_eq!(acc.usage(), 30);
        acc.account_bytes(50).unwrap();
        assert_eq!(acc.usage(), 80);
    }

    #[test]
    fn test_size_limit_exceeded() {
        let mut acc = NbtAccounter::new(50);
        acc.account_bytes(30).unwrap();
        let result = acc.account_bytes(30);
        assert!(result.is_err());
    }

    #[test]
    fn test_size_limit_exact_boundary() {
        let mut acc = NbtAccounter::new(50);
        acc.account_bytes(50).unwrap(); // exactly at limit
        let result = acc.account_bytes(1);
        assert!(result.is_err());
    }

    #[test]
    fn test_depth_tracking() {
        let mut acc = NbtAccounter::unlimited();
        assert_eq!(acc.depth(), 0);
        acc.push_depth().unwrap();
        assert_eq!(acc.depth(), 1);
        acc.push_depth().unwrap();
        assert_eq!(acc.depth(), 2);
        acc.pop_depth();
        assert_eq!(acc.depth(), 1);
        acc.pop_depth();
        assert_eq!(acc.depth(), 0);
    }

    #[test]
    fn test_depth_limit_exceeded() {
        let mut acc = NbtAccounter::new(usize::MAX);
        for _ in 0..MAX_DEPTH {
            acc.push_depth().unwrap();
        }
        assert_eq!(acc.depth(), MAX_DEPTH);
        let result = acc.push_depth();
        assert!(result.is_err());
    }

    #[test]
    fn test_default_quota_value() {
        let acc = NbtAccounter::default_quota();
        assert_eq!(acc.quota, DEFAULT_QUOTA);
    }

    #[test]
    fn test_uncompressed_quota_value() {
        let acc = NbtAccounter::uncompressed_quota();
        assert_eq!(acc.quota, UNCOMPRESSED_QUOTA);
    }

    #[test]
    fn test_unlimited_allows_large_accounting() {
        let mut acc = NbtAccounter::unlimited();
        acc.account_bytes(1_000_000_000).unwrap();
        acc.account_bytes(1_000_000_000).unwrap();
        assert_eq!(acc.usage(), 2_000_000_000);
    }

    #[test]
    fn test_saturating_add_no_panic() {
        let mut acc = NbtAccounter::unlimited();
        acc.account_bytes(usize::MAX - 1).unwrap();
        // This should saturate, not panic
        acc.account_bytes(usize::MAX - 1).unwrap();
        assert_eq!(acc.usage(), usize::MAX);
    }
}
