//! Security monitoring schema definitions
//!
//! Contains security tables:
//! - sessions: Server-side session storage for authenticated users
//! - ip_blocks: Blocked IP addresses for brute force protection
//! - rate_limit_events: Rate limiting event records
//! - step_up_auth_requests: Step-up authentication requests for sensitive actions
//! - user_devices: Known user devices for device fingerprinting
//! - compliance_digests: Weekly compliance digest records
//!
//! COMPLIANCE: §14 Compliance Monitoring System
//! COMPLIANCE: §10 Unified Database Schema Architecture
//! COMPLIANCE: §5 Observability, Logging, and Audit Requirements

pub mod indexes;
pub mod tables;

mod helpers;
mod impls;
mod types;

#[cfg(test)]
mod tests;

pub use impls::initialize_security_tables;
