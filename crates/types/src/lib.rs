//! Core domain types for `FreshCredit`
//!
//! P1 FIX: Cross-language type sync with ts-rs
//! Generate TypeScript types: cargo test --features typescript

pub mod api_response;
pub mod domain;
pub mod problem;

pub use domain::*;
pub use problem::*;
