//! The Fresh Protocol — spec traits for `AUTH → DID → DB → HASH`.
//!
//! This crate is the protocol's extension surface: the traits a deployment
//! implements to provide each stage of the invariant. It is deliberately
//! product-agnostic — no provider payloads, no review flows, no PII shapes.
//! Reference implementations live outside this crate (the libSQL/Turso
//! adapter ships in the products layer, per the vendor-neutrality rule:
//! the protocol owns the trait, products own the vendor binding).
//!
//! # The invariant
//!
//! ```text
//! AUTH → DID → DB → HASH
//! ```
//!
//! 1. **AUTH** — the user authenticates via an [`IdentityProvider`].
//! 2. **DID** — a [`DidIssuer`] issues and a [`DidVerifier`] verifies a
//!    decentralized identifier for the authenticated user. The verified DID
//!    roots the key material used to encrypt and authorize database access.
//! 3. **DB** — a per-user database is provisioned on first use through
//!    [`UserDbAdapter`]. The user is the single writer; the only follower is
//!    their encrypted backup.
//! 4. **HASH** — every committed state is anchored: [`UserDbAdapter::stage_anchor`]
//!    records an anchor intent *atomically with the write* so that no data is
//!    ever final without a block hash, and unanchored (`Pending`) state is an
//!    explicit, queryable, quarantined status — never silently final.
//!
//! # Notes for implementors
//!
//! - All traits are `Send + Sync` and async; implementations are expected to
//!   be held as `Arc<dyn …>` at composition roots.
//! - The SQL escape hatch ([`SqlDatabase`]/[`SqlConnection`]) is intentionally
//!   minimal (parameterized statements, row streams as JSON values). It is a
//!   pragmatic bridge for call sites that run raw SQL; it is not an ORM and
//!   the protocol does not grow it beyond statement execution.
//! - `backup_export` is greenfield in the reference deployment: the per-user
//!   cloud vault *is* the backup there. The trait exists so deployments can
//!   offer downloadable, hash-attested archives without a protocol change.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

mod db;
mod identity;

pub use db::{
    AdapterError, AnchorIntent, AnchorStatus, BackupArchive, ConnectionInfo, EncryptionKey,
    ProvisionedDatabase, SqlConnection, SqlDatabase, SyncStatusReport, UserDbAdapter,
};
pub use identity::{
    AuthenticationContext, AuthenticationResult, DidDocument, DidIssuer, DidVerifier,
    IdentityProvider, VerificationResult,
};
