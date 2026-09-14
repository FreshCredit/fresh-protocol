// TAG: surface=security owner=security-team rule=SEC-001
//! Operation-specific rate limiting using token bucket algorithm
//!
//! This module provides fine-grained rate limiting for high-cost operations
//! like Plaid account sync, report generation, and agent queries.

use serde::{Deserialize, Serialize};
use std::time::Duration;
use thiserror::Error;

/// Maximum number of token buckets to store (P1-PERF: Prevents unbounded growth)
pub(crate) const MAX_BUCKETS: usize = 100_000;

/// Operations that have specific rate limits
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RateLimitedOperation {
    /// Plaid account sync - expensive external API call
    /// Default: 2 per hour per user
    PlaidAccountSync,

    /// Report generation - resource intensive
    /// Default: 5 per hour per user
    ReportGeneration,

    /// Agent query - moderate cost
    /// Default: 30 per minute per user
    AgentQuery,

    /// Offer matching - moderate cost
    // TAG: surface=security owner=platform-team rule=MID-001
    /// Default: 60 per minute per user
    OfferMatch,

    /// Blockchain anchoring - external chain interaction
    /// Default: 10 per hour per user
    BlockchainAnchor,
}

impl RateLimitedOperation {
    /// Get the default configuration for this operation
    #[must_use]
    pub const fn default_config(&self) -> OperationRateLimitConfig {
        match self {
            Self::PlaidAccountSync => OperationRateLimitConfig {
                max_tokens: 2,
                refill_rate: 2,
                refill_interval: Duration::from_secs(3600), // 1 hour
                burst_size: 2,
            },
            Self::ReportGeneration => OperationRateLimitConfig {
                max_tokens: 5,
                refill_rate: 5,
                refill_interval: Duration::from_secs(3600), // 1 hour
                burst_size: 5,
            },
            Self::AgentQuery => OperationRateLimitConfig {
                max_tokens: 30,
                refill_rate: 30,
                refill_interval: Duration::from_secs(60), // 1 minute
                // TAG: surface=security owner=platform-team rule=MID-001
                burst_size: 10, // Allow small bursts
            },
            Self::OfferMatch => OperationRateLimitConfig {
                max_tokens: 60,
                refill_rate: 60,
                refill_interval: Duration::from_secs(60), // 1 minute
                burst_size: 20,
            },
            Self::BlockchainAnchor => OperationRateLimitConfig {
                max_tokens: 10,
                refill_rate: 10,
                refill_interval: Duration::from_secs(3600), // 1 hour
                burst_size: 5,
            },
        }
    }

    /// Get human-readable name for error messages
    #[must_use]
    pub const fn display_name(&self) -> &'static str {
        match self {
            Self::PlaidAccountSync => "account sync",
            Self::ReportGeneration => "report generation",
            Self::AgentQuery => "AI assistant query",
            Self::OfferMatch => "offer matching",
            Self::BlockchainAnchor => "blockchain anchoring",
        }
    }
}
// TAG: surface=security owner=platform-team rule=MID-001

/// Configuration for operation-specific rate limiting
#[derive(Debug, Clone)]
pub struct OperationRateLimitConfig {
    /// Maximum tokens in the bucket
    pub max_tokens: u32,
    /// Number of tokens to add per refill
    pub refill_rate: u32,
    /// How often to refill tokens
    pub refill_interval: Duration,
    /// Maximum burst size (tokens consumed at once)
    pub burst_size: u32,
}

/// Error type for operation rate limiting
#[derive(Debug, Error)]
pub enum OperationRateLimitError {
    /// Rate limit exceeded for the operation.
    #[error("Rate limit exceeded for {operation}: please wait {retry_after_secs} seconds before trying {operation} again")]
    LimitExceeded {
        /// Name of the rate-limited operation.
        operation: String,
        /// Seconds to wait before retrying.
        retry_after_secs: u64,
        /// Remaining tokens in the bucket.
        remaining_tokens: u32,
        /// Maximum tokens for the operation.
        max_tokens: u32,
    },
    // TAG: surface=security owner=platform-team rule=GENERAL-001
}
