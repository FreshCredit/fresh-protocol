//! Database operations for FreshCredit unified schema
//!
//! Module structure (REFACTORING IN PROGRESS):
//! - mod.rs (this file): Module exports
//! - profile.rs: User profile operations ✅ EXTRACTED
//! - financial.rs: Account and transaction operations ✅ EXTRACTED
//! - user.rs: User preferences operations
//! - report.rs: Financial report operations
//! - file.rs: File upload operations
//! - conversation.rs: AI conversation operations ✅ EXTRACTED
//! - workflow.rs: Workflow and scoring model operations ✅ EXTRACTED
//! - webhook.rs: Webhook event operations ✅ EXTRACTED
//!
//! NOTE: Operations are currently defined in lib.rs LocalClient impl block.
//! This module will contain extracted operations once refactoring is complete.
//!
//! COMPLIANCE: §10 Unified Database Schema Architecture

// Operation submodules - extracted from lib.rs LocalClient impl block
pub mod conversation;
pub mod financial;
pub mod profile;
pub mod webhook;
pub mod workflow;

// Remaining operations still in lib.rs (to be extracted):
// pub mod user;
// pub mod report;
// pub mod file;

