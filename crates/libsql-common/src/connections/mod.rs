// TAG: surface=database owner=platform-team rule=DB-001 test-coverage=unit
//! Concrete implementations of database connections
//!
//! # Feature Gates
//!
//! - `RemoteConnection` and `LocalConnection` are always available (default features)
//! - `ReplicaConnection` requires the `embedded-replica` feature (opt-in)

mod local;
mod reconnecting;
mod remote;

#[cfg(feature = "embedded-replica")]
mod replica;

pub use local::LocalConnection;
pub use reconnecting::{is_hrana_stream_error, ReconnectingConnection};
pub use remote::RemoteConnection;

#[cfg(feature = "embedded-replica")]
pub use replica::ReplicaConnection;
