//! Concrete implementations of database connections
//!
//! # Feature Gates
//!
//! - `RemoteConnection` and `LocalConnection` are always available (default features)
//! - `ReplicaConnection` requires the `embedded-replica` feature (opt-in)

mod local;
mod remote;

#[cfg(feature = "embedded-replica")]
mod replica;

pub use local::LocalConnection;
pub use remote::RemoteConnection;

#[cfg(feature = "embedded-replica")]
pub use replica::ReplicaConnection;
