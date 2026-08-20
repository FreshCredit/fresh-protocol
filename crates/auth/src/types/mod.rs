// TAG: surface=auth owner=security-team rule=RBAC-001 test-coverage=unit
//! Core authentication types
//!
//! This module defines the shared types used across the `FreshCredit` authentication system.

mod helpers;
mod impls;
#[cfg(test)]
#[allow(clippy::module_inception)]
mod tests;
#[allow(clippy::module_inception)]
mod types;

pub use types::*;
