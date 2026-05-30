//! Circuit Breaker Pattern for Database Connections
//!
//! This module provides a circuit breaker implementation that wraps database connections
//! to prevent cascading failures when the database becomes unavailable.
//!
// TAG: surface=database owner=platform-team rule=DB-001
//! # Circuit Breaker States
//!
//! ```text
//! ┌─────────────┐     failure threshold exceeded     ┌─────────────┐
//! │   CLOSED    │ ─────────────────────────────────► │    OPEN     │
//! │ (normal ops)│                                    │(failing fast)│
//! └──────┬──────┘                                    └──────┬──────┘
//!        │                                                  │
//!        │ success                                          │ timeout expired
//!        │                                                  ▼
//!        │                                         ┌─────────────┐
//!        └──────────────────────────────────────── │  HALF_OPEN  │
//!                                                  │ (test call) │
//!                                                  └──────┬──────┘
//!                                                         │
//!                              success ───────────────────┘
//!                              failure ───────────────────► OPEN
//! ```
//!
//! # Example
//!
//! ```rust,no_run
//! use freshcredit_libsql_common::{
//!     ConnectionFactory, CircuitBreakerConfig, CircuitBreakerConnection, DatabaseConnection
//! };
//!
//! async fn example() -> anyhow::Result<()> {
//!     // Create underlying connection
// TAG: surface=database owner=platform-team rule=GENERAL-001
//!     let inner = ConnectionFactory::from_env().await?;
//!
//!     // Wrap with circuit breaker
//!     let cb_config = CircuitBreakerConfig {
//!         failure_threshold: 5,
//!         recovery_timeout_secs: 30,
//!         half_open_max_calls: 3,
//!         success_threshold: 2,
//!     };
//!     let db = CircuitBreakerConnection::new(inner, cb_config);
//!
//!     // Use normally - circuit breaker handles failures automatically
//!     let rows = db.query("SELECT * FROM users", vec![]).await?;
//!
//!     Ok(())
//! }
//! ```

use async_trait::async_trait;
use std::fmt;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

use crate::connection::{ConnectionHealth, ConnectionMode, DatabaseConnection};

// TAG: surface=database owner=platform-team rule=DB-001
/// Circuit breaker states
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CircuitBreakerState {
    /// Normal operation - requests pass through
    #[default]
    Closed,
    /// Failing fast - requests immediately rejected
    Open,
    /// Testing if service has recovered
    HalfOpen,
}

/// Circuit breaker implementations
pub mod impls;
pub use impls::*;
