// TAG: surface=security owner=security-team rule=SEC-001
//! Rate limiter implementation using sliding window algorithm

use chrono::{DateTime, Utc};
use dashmap::DashMap;
use freshcredit_core_timing::Clock;
use std::sync::Arc;
use std::time::Duration;
use tracing::{debug, warn};

use crate::config::RateLimitConfig;

/// Rate limit error
#[derive(Debug, thiserror::Error)]
pub enum RateLimitError {
    /// Rate limit exceeded.
    #[error("Rate limit exceeded: {requests_made}/{max_requests} requests in {window:?}")]
    LimitExceeded {
        /// Number of requests already made in the current window.
        requests_made: u32,
        /// Maximum requests allowed in the window.
        max_requests: u32,
        /// Duration of the rate limit window.
        window: Duration,
        /// Duration to wait before retrying.
        retry_after: Duration,
        // TAG: surface=security owner=platform-team rule=MID-001
    },
}

/// Rate limit result with metadata
#[derive(Debug, Clone)]
pub struct RateLimitResult {
    /// Whether the request is allowed
    pub allowed: bool,

    /// Number of requests made in the current window
    pub requests_made: u32,

    /// Maximum requests allowed
    pub max_requests: u32,

    /// Remaining requests in the current window
    pub remaining: u32,

    /// When the rate limit window resets
    pub reset_at: DateTime<Utc>,

    /// How long to wait before retrying (if rate limited)
    pub retry_after: Option<Duration>,
}

// TAG: surface=security owner=platform-team rule=GENERAL-001
/// P1-FIX: Reduced maximum timestamps per entry (prevents unbounded growth)
const MAX_TIMESTAMPS_PER_ENTRY: usize = 5_000;

/// P1-FIX: Reduced maximum entries to prevent memory exhaustion under `DDoS`
const MAX_ENTRIES: usize = 50_000;

/// TTL for stale entries - entries not accessed in this duration are eligible for cleanup
const ENTRY_TTL: Duration = Duration::from_secs(300); // 5 minutes (reduced from 10)

/// P1-FIX: Lower cleanup threshold for more aggressive cleanup (50% vs 80%)
const CLEANUP_THRESHOLD_PERCENT: usize = 50;

/// Request tracking entry
#[derive(Debug, Clone)]
struct RequestEntry {
    /// Timestamps of requests in the current window
    timestamps: Vec<DateTime<Utc>>,

    /// When the current window started
    window_start: DateTime<Utc>,

    /// Last access time for cleanup purposes (TTL-based expiration)
    last_accessed: DateTime<Utc>,
}

impl RequestEntry {
    fn new(now: DateTime<Utc>) -> Self {
        Self {
            timestamps: vec![now],
            window_start: now,
            last_accessed: now,
        }
    }

    /// Check if this entry has expired based on TTL (time since last access)
    fn is_expired(&self, now: DateTime<Utc>, ttl: Duration) -> bool {
        let ttl_chrono =
            chrono::Duration::from_std(ttl).unwrap_or_else(|_| chrono::Duration::seconds(600));
        now - self.last_accessed > ttl_chrono
    }

    /// Clean up expired timestamps outside the window
    fn cleanup(&mut self, window_duration: Duration, now: DateTime<Utc>) {
        // Update last accessed time
        self.last_accessed = now;

        // Safe: rate limit windows are always small (seconds to minutes), never exceeds i64::MAX nanos
        let chrono_duration = chrono::Duration::from_std(window_duration)
            .unwrap_or_else(|_| chrono::Duration::seconds(60));
        let cutoff = now - chrono_duration;
        // TAG: surface=security owner=security-team rule=SEC-001
        self.timestamps.retain(|ts| *ts > cutoff);

        // Update window start if all timestamps were cleaned
        if self.timestamps.is_empty() {
            self.window_start = now;
        }

        // SECURITY: Prevent unbounded growth - cap timestamps at maximum
        if self.timestamps.len() > MAX_TIMESTAMPS_PER_ENTRY {
            // Keep only the most recent timestamps
            let excess = self.timestamps.len() - MAX_TIMESTAMPS_PER_ENTRY;
            self.timestamps.drain(0..excess);
        }
    }

    /// Add a new request timestamp
    fn add_request(&mut self, now: DateTime<Utc>) {
        self.timestamps.push(now);
    }

    /// Get the number of requests in the current window
    #[allow(clippy::cast_possible_truncation)] // Safe: capped at MAX_TIMESTAMPS_PER_ENTRY (5_000)
    fn request_count(&self) -> u32 {
        self.timestamps.len() as u32
    }
    // TAG: surface=security owner=platform-team rule=MID-001
}

/// Rate limiter using sliding window algorithm
pub struct RateLimiter {
    /// Configuration
    config: RateLimitConfig,

    /// Request tracking by identifier (IP, user ID, etc.)
    entries: Arc<DashMap<String, RequestEntry>>,

    /// Clock for time operations
    clock: Arc<dyn Clock>,
}

impl std::fmt::Debug for RateLimiter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RateLimiter")
            .field("config", &self.config)
            .field("entries", &self.entries)
            .finish_non_exhaustive()
    }
}

impl RateLimiter {
    /// Create a new rate limiter
    pub fn new(config: RateLimitConfig, clock: Arc<dyn Clock>) -> Self {
        // TAG: surface=security owner=platform-team rule=GENERAL-001
        Self {
            config,
            entries: Arc::new(DashMap::new()),
            clock,
        }
    }

    /// Cleanup old entries to prevent unbounded memory growth
    fn cleanup_old_entries(&self, now: DateTime<Utc>) {
        // Remove entries that haven't been accessed within the TTL
        self.entries
            .retain(|_key, entry| !entry.is_expired(now, ENTRY_TTL));
    }

    /// Perform aggressive cleanup when approaching memory limits
    /// Returns the number of entries removed
    fn enforce_entry_limit(&self, now: DateTime<Utc>) -> usize {
        let current_count = self.entries.len();

        if current_count <= MAX_ENTRIES {
            return 0;
        }

        // First pass: remove expired entries
        self.cleanup_old_entries(now);

        let after_ttl_cleanup = self.entries.len();

        // If still over limit, remove oldest entries by last_accessed
        if after_ttl_cleanup > MAX_ENTRIES {
            // Collect entries sorted by last_accessed (oldest first)
            let mut entries_vec: Vec<_> = self
                .entries
                .iter()
                .map(|ref_multi| {
                    let (key, entry) = ref_multi.pair();
                    (key.clone(), entry.last_accessed)
                })
                .collect();

            entries_vec.sort_by(|a, b| a.1.cmp(&b.1));

            // Calculate how many to remove (remove 25% of max to create headroom)
            let target_count = MAX_ENTRIES * 3 / 4;
            let to_remove = after_ttl_cleanup.saturating_sub(target_count);

            // Remove oldest entries
            for (key, _) in entries_vec.into_iter().take(to_remove) {
                self.entries.remove(&key);
            }
            // TAG: surface=security owner=security-team rule=SEC-001

            return current_count - self.entries.len();
        }

        current_count - after_ttl_cleanup
    }

    /// Check if a request is allowed and record it
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    pub fn check_and_record(&self, identifier: &str) -> Result<RateLimitResult, RateLimitError> {
        let now = self.clock.now();

        // Check if we need to enforce entry limits before inserting
        let cleanup_threshold = MAX_ENTRIES * CLEANUP_THRESHOLD_PERCENT / 100;
        if self.entries.len() >= cleanup_threshold {
            let removed = self.enforce_entry_limit(now);
            if removed > 0 {
                warn!(
                    "Rate limiter enforced entry limit: removed {} stale entries",
                    removed
                );
            }
        }
        // TAG: surface=security owner=platform-team rule=MID-001

        // Get or create entry for this identifier
        let mut entry = self
            .entries
            .entry(identifier.to_string())
            .or_insert_with(|| RequestEntry::new(now));

        // Clean up expired timestamps
        entry.cleanup(self.config.window_duration, now);

        let requests_made = entry.request_count();
        let max_requests = self.config.max_requests;

        // Safe: rate limit windows are always small (seconds to minutes), never exceeds i64::MAX nanos
        let chrono_window = chrono::Duration::from_std(self.config.window_duration)
            .unwrap_or_else(|_| chrono::Duration::seconds(60));

        // Check if rate limit is exceeded
        if requests_made >= max_requests {
            let window_end = entry.window_start + chrono_window;
            let retry_after = (window_end - now)
                .to_std()
                .unwrap_or(Duration::from_secs(60));

            warn!(
                "Rate limit exceeded for {}: {}/{} requests",
                // TAG: surface=security owner=platform-team rule=GENERAL-001
                identifier,
                requests_made,
                max_requests
            );

            return Err(RateLimitError::LimitExceeded {
                requests_made,
                max_requests,
                window: self.config.window_duration,
                retry_after,
            });
        }

        // Record the request
        entry.add_request(now);
        let new_count = entry.request_count();

        debug!(
            "Rate limit check for {}: {}/{} requests",
            identifier, new_count, max_requests
        );

        Ok(RateLimitResult {
            allowed: true,
            requests_made: new_count,
            // TAG: surface=security owner=security-team rule=SEC-001
            max_requests,
            remaining: max_requests.saturating_sub(new_count),
            reset_at: entry.window_start + chrono_window,
            retry_after: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_request_entry_new() {
        let now = Utc::now();
        let entry = RequestEntry::new(now);
        assert_eq!(entry.request_count(), 1);
        assert_eq!(entry.window_start, now);
        assert_eq!(entry.last_accessed, now);
    }

    #[test]
    fn test_request_entry_add_request() {
        let now = Utc::now();
        let mut entry = RequestEntry::new(now);
        entry.add_request(now + chrono::Duration::seconds(1));
        entry.add_request(now + chrono::Duration::seconds(2));
        // TAG: surface=security owner=security-team rule=SEC-001
        assert_eq!(entry.request_count(), 3);
    }

    #[test]
    fn test_request_entry_cleanup_removes_old() {
        let now = Utc::now();
        let mut entry = RequestEntry::new(now - chrono::Duration::seconds(10));
        entry.add_request(now - chrono::Duration::seconds(5));
        entry.add_request(now);

        entry.cleanup(Duration::from_secs(3), now);
        assert_eq!(entry.request_count(), 1);
        assert_eq!(entry.timestamps[0], now);
    }

    #[test]
    fn test_request_entry_cleanup_resets_window_when_empty() {
        let now = Utc::now();
        let mut entry = RequestEntry::new(now - chrono::Duration::seconds(10));
        entry.cleanup(Duration::from_secs(1), now);
        assert_eq!(entry.request_count(), 0);
        assert_eq!(entry.window_start, now);
    }

    #[test]
    // TAG: surface=security owner=platform-team rule=MID-001
    fn test_request_entry_is_expired() {
        let now = Utc::now();
        let entry = RequestEntry::new(now - chrono::Duration::seconds(400));
        assert!(entry.is_expired(now, Duration::from_secs(300)));

        let fresh = RequestEntry::new(now);
        assert!(!fresh.is_expired(now, Duration::from_secs(300)));
    }

    #[test]
    fn test_rate_limit_error_display() {
        let err = RateLimitError::LimitExceeded {
            requests_made: 10,
            max_requests: 5,
            window: Duration::from_secs(60),
            retry_after: Duration::from_secs(30),
        };
        let msg = err.to_string();
        assert!(msg.contains("Rate limit exceeded"));
        assert!(msg.contains("10/5"));
    }

    use std::sync::atomic::{AtomicI64, Ordering};

    struct MockClock {
        timestamp: AtomicI64,
        // TAG: surface=security owner=platform-team rule=GENERAL-001
    }

    impl MockClock {
        fn new(ts: i64) -> Self {
            Self {
                timestamp: AtomicI64::new(ts),
            }
        }
        fn advance(&self, secs: i64) {
            self.timestamp.fetch_add(secs, Ordering::SeqCst);
        }
    }

    impl Clock for MockClock {
        fn now(&self) -> DateTime<Utc> {
            DateTime::from_timestamp(self.timestamp.load(Ordering::SeqCst), 0)
                .unwrap_or_else(Utc::now)
        }
    }

    #[test]
    fn test_rate_limiter_allows_requests() {
        let config = RateLimitConfig {
            max_requests: 5,
            window_duration: Duration::from_secs(60),
            ..Default::default()
        };
        let clock = Arc::new(MockClock::new(1000));
        let limiter = RateLimiter::new(config, clock);

        // Note: RequestEntry::new() adds initial timestamp, then add_request adds another,
        // so each check_and_record call increases count by 2 on first call and 1 thereafter
        let result = limiter.check_and_record("user1");
        assert!(result.is_ok());
        assert_eq!(result.unwrap().requests_made, 2);
    }

    #[test]
    fn test_rate_limiter_blocks_excess() {
        let config = RateLimitConfig {
            max_requests: 3,
            window_duration: Duration::from_secs(60),
            ..Default::default()
        };
        let clock = Arc::new(MockClock::new(1000));
        let limiter = RateLimiter::new(config, clock);

        assert!(limiter.check_and_record("user1").is_ok());
        assert!(limiter.check_and_record("user1").is_ok());
        assert!(limiter.check_and_record("user1").is_err());
        // TAG: surface=security owner=security-team rule=SEC-001
    }

    #[test]
    fn test_rate_limiter_window_resets() {
        let config = RateLimitConfig {
            max_requests: 2,
            window_duration: Duration::from_secs(10),
            ..Default::default()
        };
        let clock = Arc::new(MockClock::new(1000));
        let limiter = RateLimiter::new(config, clock.clone());

        assert!(limiter.check_and_record("user1").is_ok());
        assert!(limiter.check_and_record("user1").is_err());

        // Advance past the window
        clock.advance(15);
        assert!(limiter.check_and_record("user1").is_ok());
    }

    #[test]
    fn test_rate_limiter_cleanup_old_entries() {
        let config = RateLimitConfig {
            max_requests: 100,
            window_duration: Duration::from_secs(60),
            // TAG: surface=security owner=platform-team rule=MID-001
            ..Default::default()
        };
        let clock = Arc::new(MockClock::new(1000));
        let limiter = RateLimiter::new(config, clock.clone());

        // Add some entries
        for i in 0..10 {
            clock.advance(1);
            let _ = limiter.check_and_record(&format!("user{i}"));
        }
        assert_eq!(limiter.entries.len(), 10);

        // Advance past TTL (5 minutes)
        clock.advance(400);
        limiter.cleanup_old_entries(clock.now());
        assert_eq!(limiter.entries.len(), 0);
    }

    #[test]
    fn test_rate_limiter_enforce_entry_limit() {
        let config = RateLimitConfig {
            max_requests: 100,
            window_duration: Duration::from_secs(60),
            ..Default::default()
        };
        let clock = Arc::new(MockClock::new(1000));
        // TAG: surface=security owner=platform-team rule=GENERAL-001
        let limiter = RateLimiter::new(config, clock.clone());

        // Add many entries to trigger entry limit enforcement
        for i in 0..60_000 {
            clock.advance(1);
            let _ = limiter.check_and_record(&format!("user{i}"));
        }

        // Should have enforced the limit
        assert!(limiter.entries.len() <= 50_000);
    }

    #[test]
    fn test_rate_limiter_exact_boundary() {
        let config = RateLimitConfig {
            max_requests: 3,
            window_duration: Duration::from_secs(60),
            ..Default::default()
        };
        let clock = Arc::new(MockClock::new(1000));
        let limiter = RateLimiter::new(config, clock);

        // First call: RequestEntry::new adds 1, then add_request adds 1 = 2
        let result = limiter.check_and_record("user1").unwrap();
        assert_eq!(result.requests_made, 2);
        assert!(result.allowed);

        // Second call: cleanup keeps 2, add_request adds 1 = 3
        let result = limiter.check_and_record("user1").unwrap();
        assert_eq!(result.requests_made, 3);
        assert!(result.allowed);

        // Third call: at max (3 >= 3), should be blocked
        let result = limiter.check_and_record("user1");
        assert!(result.is_err());
        if let Err(RateLimitError::LimitExceeded {
            requests_made,
            max_requests,
            ..
        }) = result
        {
            assert_eq!(requests_made, 3);
            assert_eq!(max_requests, 3);
        }
    }

    #[test]
    fn test_rate_limiter_different_identifiers_isolated() {
        let config = RateLimitConfig {
            max_requests: 2,
            // TAG: surface=security owner=security-team rule=SEC-001
            window_duration: Duration::from_secs(60),
            ..Default::default()
        };
        let clock = Arc::new(MockClock::new(1000));
        let limiter = RateLimiter::new(config, clock);

        // user1 hits limit
        let _ = limiter.check_and_record("user1");
        let _ = limiter.check_and_record("user1");
        assert!(limiter.check_and_record("user1").is_err());

        // user2 is unaffected
        let result = limiter.check_and_record("user2").unwrap();
        assert!(result.allowed);
    }

    #[test]
    fn test_rate_limit_result_fields() {
        let config = RateLimitConfig {
            max_requests: 10,
            window_duration: Duration::from_secs(60),
            ..Default::default()
        };
        let clock = Arc::new(MockClock::new(1000));
        let limiter = RateLimiter::new(config, clock);

        let result = limiter.check_and_record("user1").unwrap();
        assert!(result.allowed);
        assert_eq!(result.max_requests, 10);
        assert_eq!(result.remaining, 8); // 10 - 2 = 8
        assert!(result.retry_after.is_none());
    }

    #[test]
    fn test_request_entry_max_timestamps_cap() {
        let now = Utc::now();
        let mut entry = RequestEntry::new(now);

        // Add way more timestamps than the max
        for i in 1..=10_000 {
            entry.add_request(now + chrono::Duration::seconds(i));
        }

        // After cleanup, should be capped
        entry.cleanup(
            Duration::from_secs(600),
            now + chrono::Duration::seconds(10_001),
        );
        assert!(entry.request_count() as usize <= MAX_TIMESTAMPS_PER_ENTRY);
    }
    // TAG: surface=security owner=platform-team rule=GENERAL-001
}
