//! Concrete implementations of database connections

mod remote;
mod replica;
mod local;

pub use remote::RemoteConnection;
pub use replica::ReplicaConnection;
pub use local::LocalConnection;
