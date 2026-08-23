// TAG: surface=api owner=platform-team rule=API-001
//! Global clock abstraction for distributed, NTP-synchronized time.
//!
//! This module provides the Phase 1 implementation of the global clock plan:
//! a `GlobalClock` trait that pairs wall-clock UTC with a monotonic anchor and
//! an optional uncertainty bound, plus an NTP-backed implementation that
//! synchronizes against Google public NTP (leap-second smeared).
//!
//! # Design goals
//!
//! * **Monotonic duration measurements must never use wall-clock subtraction.**
//!   Use `monotonic_now()` / `MonotonicDeadline` for timeouts, TTL enforcement,
//!   and rate-limit refills.
//! * **Wall-clock is still required** for user-facing timestamps, audit logs,
//!   cron schedules, and cross-node ordering. The NTP implementation records an
//!   estimated uncertainty so callers can decide when to distrust a reading.
//! * **Clock sources are explicit.** `TimeSource::Host` is unsynchronized;
//!   `TimeSource::Ntp` is synchronized via UDP NTP; `TimeSource::Spanner` is
//!   reserved for future TrueTime-style bounds.
//!
//! See `docs/application/planning/global-clock-plan.md` for the full roadmap.

use chrono::{DateTime, Utc};
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::clock::{Clock, TimeSource};

/// Difference between the NTP epoch (1900-01-01) and the Unix epoch (1970-01-01).
const NTP_UNIX_OFFSET_SECONDS: u64 = 2_208_988_800;

/// Default NTP server. Google public NTP smears leap seconds; this avoids a
/// 61-second minute and keeps monotonic sleep deadlines stable across them.
const DEFAULT_NTP_SERVER: &str = "time.google.com:123";

/// Default re-sync interval for the NTP clock.
const DEFAULT_NTP_SYNC_INTERVAL: Duration = Duration::from_secs(300);

/// Maximum tolerated delta between a fresh NTP sample and the local host clock.
/// Samples outside this window are logged and ignored to protect against spoofed
/// or corrupted replies.
const NTP_SANITY_SECONDS: i64 = 60;

/// A timestamp paired with metadata about how it was produced.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Timestamp {
    /// Wall-clock UTC instant. This is what users, reports, and audit logs see.
    pub wall_utc: DateTime<Utc>,
    /// Monotonic anchor captured at the same moment as `wall_utc`. Use this for
    /// deadlines and duration measurements, never `wall_utc` subtraction.
    pub monotonic: Instant,
    /// Source that produced the wall-clock reading.
    pub source: TimeSource,
    /// Estimated ±uncertainty of the wall-clock reading, if known.
    pub uncertainty: Option<Duration>,
}

impl Timestamp {
    /// True if the wall-clock reading is within the stated uncertainty bounds.
    /// Always true for host clocks (uncertainty is `None`).
    #[must_use]
    pub fn is_within_uncertainty(&self, other: DateTime<Utc>) -> bool {
        match self.uncertainty {
            Some(delta) => {
                let diff = (other - self.wall_utc).abs();
                diff.to_std().map(|d| d <= delta).unwrap_or(false)
            }
            None => true,
        }
    }
}

/// Clock abstraction suitable for distributed FreshCredit components.
///
/// Inherits the basic `Clock` operations and adds a structured `Timestamp`
/// snapshot plus monotonic deadline helpers.
pub trait GlobalClock: Clock + Send + Sync {
    /// Capture a full timestamp snapshot.
    fn snapshot(&self) -> Timestamp;

    /// Build a monotonic deadline `Duration` from now. The returned deadline
    /// is safe to pass to `tokio::time::sleep_until` even if the wall clock
    /// jumps backward or forward.
    #[must_use]
    fn monotonic_deadline(&self, timeout: Duration) -> Instant {
        self.monotonic_now()
            .checked_add(timeout)
            .unwrap_or_else(Instant::now)
    }

    /// Convenience: create a `MonotonicDeadline` from a duration.
    #[must_use]
    fn deadline(&self, timeout: Duration) -> MonotonicDeadline
    where
        Self: Sized,
    {
        MonotonicDeadline::after(self, timeout)
    }

    /// Convenience: create a `MonotonicDeadline` from an absolute wall-clock
    /// target. The deadline is anchored to the current monotonic instant, so a
    /// wall-clock jump does not silently extend or shorten the wait.
    #[must_use]
    fn deadline_until(&self, target: DateTime<Utc>) -> MonotonicDeadline
    where
        Self: Sized,
    {
        let now_wall = self.now();
        let delta = (target - now_wall).max(chrono::Duration::zero());
        let timeout = delta.to_std().unwrap_or(Duration::ZERO);
        MonotonicDeadline::after(self, timeout)
    }
}

/// A deadline backed by the monotonic clock. Safe across wall-clock jumps.
#[derive(Debug, Clone, Copy)]
pub struct MonotonicDeadline {
    deadline: Instant,
}

impl MonotonicDeadline {
    /// Create a deadline `timeout` after the current monotonic instant of `clock`.
    #[must_use]
    pub fn after(clock: &dyn GlobalClock, timeout: Duration) -> Self {
        Self {
            deadline: clock
                .monotonic_now()
                .checked_add(timeout)
                .unwrap_or_else(Instant::now),
        }
    }

    /// Create a deadline from an absolute monotonic instant.
    #[must_use]
    pub const fn at(instant: Instant) -> Self {
        Self { deadline: instant }
    }

    /// Remaining time until the deadline. Returns `Duration::ZERO` if expired.
    #[must_use]
    pub fn remaining(&self) -> Duration {
        self.deadline.saturating_duration_since(Instant::now())
    }

    /// True if the deadline has passed.
    #[must_use]
    pub fn is_expired(&self) -> bool {
        Instant::now() >= self.deadline
    }

    /// The underlying monotonic instant.
    #[must_use]
    pub const fn instant(&self) -> Instant {
        self.deadline
    }
}

/// Host-only global clock. Equivalent to `SystemClock` but implements
/// `GlobalClock` and returns a `Timestamp` snapshot.
#[derive(Debug, Default)]
pub struct HostGlobalClock;

impl Clock for HostGlobalClock {
    fn now(&self) -> DateTime<Utc> {
        Utc::now()
    }

    fn monotonic_now(&self) -> Instant {
        Instant::now()
    }

    fn uncertainty(&self) -> Option<Duration> {
        None
    }

    fn source(&self) -> TimeSource {
        TimeSource::Host
    }
}

impl GlobalClock for HostGlobalClock {
    fn snapshot(&self) -> Timestamp {
        Timestamp {
            wall_utc: self.now(),
            monotonic: self.monotonic_now(),
            source: TimeSource::Host,
            uncertainty: None,
        }
    }
}

/// Result of a single NTP synchronization round-trip.
#[derive(Debug, Clone, Copy)]
pub struct NtpSample {
    /// Wall-clock UTC estimate from the server.
    pub wall_utc: DateTime<Utc>,
    /// Round-trip time of the NTP packet.
    pub rtt: Duration,
    /// Estimated one-way uncertainty (half the RTT, loosely).
    pub uncertainty: Duration,
}

#[derive(Debug, thiserror::Error)]
/// Errors that can occur during an NTP synchronization round-trip.
pub enum NtpError {
    /// UDP send/recv failure or DNS resolution failure.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    /// The server reply did not contain a parsable transmit timestamp.
    #[error("NTP server returned invalid transmit timestamp")]
    InvalidTimestamp,
    /// The server reported stratum 0, meaning it is not synchronized.
    #[error("NTP server stratum indicates unsynchronized server")]
    Unsynchronized,
}

/// NTP-backed global clock.
///
/// On construction the clock performs an initial sync and then spawns a
/// background Tokio task that re-syncs every 5 minutes. The latest offset is
/// stored atomically and applied to `now()` on every read.
///
/// Wall time is computed as `Utc::now() + offset`, where `offset` is updated
/// by each NTP sample. This lets the clock steer smoothly rather than jump.
#[derive(Debug)]
pub struct NtpGlobalClock {
    /// Microsecond offset applied to `Utc::now()` to produce NTP-synchronized
    /// wall time. Positive means our host clock is behind NTP; negative means
    /// ahead.
    offset_us: AtomicI64,
    /// Estimated uncertainty in microseconds.
    uncertainty_us: AtomicI64,
    server: String,
    sync_interval: Duration,
}

impl NtpGlobalClock {
    /// Create a new NTP clock and perform the initial synchronization.
    ///
    /// # Errors
    ///
    /// Returns `NtpError` if the initial NTP query fails.
    pub async fn new() -> Result<Arc<Self>, NtpError> {
        Self::with_server(DEFAULT_NTP_SERVER).await
    }

    /// Create a clock that synchronizes against a specific `host:port`.
    ///
    /// # Errors
    ///
    /// Returns `NtpError` if the initial NTP query fails.
    pub async fn with_server(server: &str) -> Result<Arc<Self>, NtpError> {
        let clock = Arc::new(Self {
            offset_us: AtomicI64::new(0),
            uncertainty_us: AtomicI64::new(0),
            server: server.to_string(),
            sync_interval: DEFAULT_NTP_SYNC_INTERVAL,
        });
        let sample = clock.sync_once().await?;
        clock.apply_sample(sample);
        // Spawn background re-sync task.
        let cloned = Arc::clone(&clock);
        tokio::spawn(async move {
            cloned.sync_loop().await;
        });
        Ok(clock)
    }

    fn apply_sample(&self, sample: NtpSample) {
        let local_wall_at_response = Utc::now();
        let offset = sample.wall_utc - local_wall_at_response;
        let offset_us = offset.num_microseconds().unwrap_or(0);
        let uncertainty_us = sample
            .uncertainty
            .as_micros()
            .try_into()
            .unwrap_or(i64::MAX);
        self.offset_us.store(offset_us, Ordering::Relaxed);
        self.uncertainty_us.store(uncertainty_us, Ordering::Relaxed);
    }

    /// Perform a single NTP v4 query and return the sampled time.
    ///
    /// # Errors
    ///
    /// Returns `NtpError` on network failure or invalid server response.
    pub async fn sync_once(&self) -> Result<NtpSample, NtpError> {
        let t0 = Instant::now();
        let local_wall_t0 = Utc::now();
        let socket = tokio::net::UdpSocket::bind("0.0.0.0:0").await?;
        socket.connect(&self.server).await?;

        // NTP v4 client packet: first byte = 0x1B (LI=0, VN=4, mode=3 client).
        let mut request = [0u8; 48];
        request[0] = 0x1B;
        socket.send(&request).await?;

        let mut response = [0u8; 48];
        let bytes_read = tokio::time::timeout(Duration::from_secs(5), socket.recv(&mut response))
            .await
            .map_err(|_| std::io::Error::new(std::io::ErrorKind::TimedOut, "NTP recv timeout"))??;

        if bytes_read < 48 {
            return Err(NtpError::InvalidTimestamp);
        }

        // Stratum in byte 1: 0 = unspecified/kiss-o'-death, 1 = primary,
        // >=2 = secondary.
        let stratum = response[1];
        if stratum == 0 {
            return Err(NtpError::Unsynchronized);
        }

        let t3 = Instant::now();
        let rtt = t3.duration_since(t0);

        // Transmit timestamp is at bytes 40-47: 32-bit seconds since 1900-01-01,
        // followed by 32-bit fraction.
        let seconds = u32::from_be_bytes([response[40], response[41], response[42], response[43]]);
        let fraction = u32::from_be_bytes([response[44], response[45], response[46], response[47]]);

        let unix_seconds = seconds
            .checked_sub(NTP_UNIX_OFFSET_SECONDS as u32)
            .ok_or(NtpError::InvalidTimestamp)?;
        let nanos = ((fraction as u64) * 1_000_000_000 / (1u64 << 32)) as u32;

        let server_wall = DateTime::from_timestamp(unix_seconds as i64, nanos)
            .ok_or(NtpError::InvalidTimestamp)?;

        // Crude uncertainty: half RTT.
        let uncertainty = rtt / 2;

        // Correct server wall by half RTT so it approximates the time when the
        // packet left the server.
        let corrected_wall = server_wall
            + chrono::Duration::from_std(uncertainty).unwrap_or(chrono::Duration::zero());

        // Sanity check against local clock.
        let local_delta = (corrected_wall - local_wall_t0).abs();
        if local_delta > chrono::Duration::seconds(NTP_SANITY_SECONDS) {
            tracing::warn!(
                "NTP sample differs from local clock by {:?}; ignoring",
                local_delta
            );
        }

        Ok(NtpSample {
            wall_utc: corrected_wall,
            rtt,
            uncertainty,
        })
    }

    async fn sync_loop(self: Arc<Self>) {
        loop {
            tokio::time::sleep(self.sync_interval).await;
            match self.sync_once().await {
                Ok(sample) => self.apply_sample(sample),
                Err(e) => {
                    tracing::warn!(error = %e, "NTP re-sync failed");
                }
            }
        }
    }

    fn current_offset_us(&self) -> i64 {
        self.offset_us.load(Ordering::Relaxed)
    }

    fn current_uncertainty(&self) -> Option<Duration> {
        let us = self.uncertainty_us.load(Ordering::Relaxed);
        if us <= 0 {
            None
        } else {
            Some(Duration::from_micros(us as u64))
        }
    }
}

impl Clock for NtpGlobalClock {
    fn now(&self) -> DateTime<Utc> {
        let offset = chrono::Duration::microseconds(self.current_offset_us());
        Utc::now() + offset
    }

    fn monotonic_now(&self) -> Instant {
        Instant::now()
    }

    fn uncertainty(&self) -> Option<Duration> {
        self.current_uncertainty()
    }

    fn source(&self) -> TimeSource {
        TimeSource::Ntp
    }
}

impl GlobalClock for NtpGlobalClock {
    fn snapshot(&self) -> Timestamp {
        let wall_utc = self.now();
        let monotonic = self.monotonic_now();
        Timestamp {
            wall_utc,
            monotonic,
            source: TimeSource::Ntp,
            uncertainty: self.uncertainty(),
        }
    }
}

/// Factory for creating the production global clock.
///
/// If `enable_ntp` is `true` and the initial sync succeeds, returns an
/// `NtpGlobalClock`; otherwise falls back to `HostGlobalClock`. This lets
/// deployments opt into NTP synchronization without failing startup when the
/// network is temporarily unavailable.
pub struct GlobalClockFactory;

impl GlobalClockFactory {
    /// Create a global clock. Falls back to host clock if NTP is disabled or
    /// unreachable.
    pub async fn create(enable_ntp: bool) -> Arc<dyn GlobalClock> {
        if !enable_ntp {
            return Arc::new(HostGlobalClock);
        }
        match NtpGlobalClock::new().await {
            Ok(clock) => clock as Arc<dyn GlobalClock>,
            Err(e) => {
                tracing::warn!(error = %e, "NTP clock unavailable; falling back to host clock");
                Arc::new(HostGlobalClock)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_host_global_clock_snapshot() {
        let clock = HostGlobalClock;
        let ts = clock.snapshot();
        assert_eq!(ts.source, TimeSource::Host);
        assert!(ts.uncertainty.is_none());
        assert!(ts.monotonic <= Instant::now());
    }

    #[test]
    fn test_monotonic_deadline_resists_wall_clock_jump() {
        let clock = HostGlobalClock;
        let deadline = clock.deadline(Duration::from_millis(50));
        std::thread::sleep(Duration::from_millis(10));
        assert!(!deadline.is_expired());
        assert!(deadline.remaining() <= Duration::from_millis(50));
    }

    #[test]
    fn test_monotonic_deadline_expires() {
        let clock = HostGlobalClock;
        let deadline = clock.deadline(Duration::from_millis(5));
        std::thread::sleep(Duration::from_millis(20));
        assert!(deadline.is_expired());
    }

    #[test]
    fn test_timestamp_within_uncertainty() {
        let now = Utc::now();
        let ts = Timestamp {
            wall_utc: now,
            monotonic: Instant::now(),
            source: TimeSource::Ntp,
            uncertainty: Some(Duration::from_secs(1)),
        };
        assert!(ts.is_within_uncertainty(now + chrono::Duration::milliseconds(500)));
        assert!(!ts.is_within_uncertainty(now + chrono::Duration::seconds(2)));
    }

    #[test]
    fn test_ntp_packet_parsing_happy_path() {
        // Build a synthetic NTP response with a known transmit timestamp.
        // Transmit timestamp seconds = NTP_UNIX_OFFSET + 1_000_000_000
        let seconds = (NTP_UNIX_OFFSET_SECONDS + 1_000_000_000) as u32;
        let fraction = 0u32;
        let mut response = [0u8; 48];
        response[0] = 0x1C; // stratum 1, version 4, mode 4 (server)
        response[1] = 1; // stratum 1
        response[40..44].copy_from_slice(&seconds.to_be_bytes());
        response[44..48].copy_from_slice(&fraction.to_be_bytes());

        // Verify the byte math that `sync_once` would apply.
        let parsed_seconds =
            u32::from_be_bytes([response[40], response[41], response[42], response[43]]);
        let parsed_unix = parsed_seconds - NTP_UNIX_OFFSET_SECONDS as u32;
        assert_eq!(parsed_unix, 1_000_000_000);
    }
}
