//! Concrete implementations of database connections
//!
//! # Feature Gates
//!
//! - `RemoteConnection` and `LocalConnection` are always available (default features)
//! - `ReplicaConnection` requires the `embedded-replica` feature (opt-in)

mod remote;
mod local;

#[cfg(feature = "embedded-replica")]
mod replica;

pub use remote::RemoteConnection;
pub use local::LocalConnection;

#[cfg(feature = "embedded-replica")]
pub use replica::ReplicaConnection;
