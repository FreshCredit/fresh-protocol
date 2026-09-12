//! Stage-3/4 spec traits: the per-user database adapter and the hash-commit
//! hook surface.

use std::fmt::Debug;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Errors every adapter operation can produce.
#[derive(Debug, thiserror::Error)]
pub enum AdapterError {
    /// Configuration is missing or invalid (env vars, credentials, URLs).
    #[error("adapter configuration error: {0}")]
    Config(String),

    /// The backend service is unavailable (network, quota, circuit open).
    #[error("backend unavailable: {0}")]
    Unavailable(String),

    /// The requested user database does not exist (and `provision` was not
    /// requested or is not permitted on this path).
    #[error("user database not found: {0}")]
    NotFound(String),

    /// The backend rejected the operation; carries the backend message.
    #[error("backend error: {0}")]
    Backend(String),

    /// The operation is not supported by this adapter configuration (e.g.
    /// cloud provisioning disabled, BYOK not enabled, `backup_export`
    /// unimplemented in the reference adapter).
    #[error("operation not supported: {0}")]
    Unsupported(String),
}

/// Outcome of provisioning a user's database.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProvisionedDatabase {
    /// Stable logical name of the database (deployment-defined; pure naming,
    /// no I/O).
    pub database_name: String,

    /// Endpoint the database is reachable at, if remote.
    pub url: Option<String>,

    /// Whether the database was created by this call (`false` = existed).
    pub created: bool,

    /// Whether remote encryption (bring-your-own-key) is active for it.
    pub encrypted: bool,
}

/// Seals extensible structs against exhaustive construction outside the
/// adapter implementations that build them.
#[derive(Debug, Clone)]
pub struct NonExhaustivePrivate {
    _private: (),
}

impl NonExhaustivePrivate {
    pub(crate) const fn new() -> Self {
        Self { _private: () }
    }
}

/// Short-lived connection material for opening a user's remote database.
///
/// TTL/caching semantics (token lifetime, refresh margins, connection
/// pooling) are an implementation detail of the adapter; callers treat each
/// `ConnectionInfo` as opaque and short-lived.
#[derive(Debug, Clone)]
pub struct ConnectionInfo {
    /// Endpoint to connect to.
    pub url: String,

    /// Bearer/token material scoped to this user and database.
    pub token: String,

    /// Seconds until `token` expires; implementations mint with margin so a
    /// value of `None` means "no documented expiry".
    pub expires_in_secs: Option<u64>,

    _private: NonExhaustivePrivate,
}

impl ConnectionInfo {
    /// Construct a `ConnectionInfo` (adapter implementations only).
    #[must_use]
    pub const fn new(url: String, token: String, expires_in_secs: Option<u64>) -> Self {
        Self {
            url,
            token,
            expires_in_secs,
            _private: NonExhaustivePrivate::new(),
        }
    }
}

/// Resolved encryption material for a user's database.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncryptionKey {
    /// Base64-encoded key bytes.
    pub key_b64: String,

    /// Cipher identifier (e.g. `aegis256`).
    pub cipher: String,

    /// Whether the database is actually encrypted with this key. When
    /// `false`, the key is a no-op placeholder and MUST NOT be persisted
    /// into the database by callers.
    pub encrypted: bool,

    /// Key-envelope version, if the adapter versions its envelopes
    /// (rotation/migration bookkeeping).
    pub key_version: Option<u32>,
}

/// Liveness/readiness view of a user's synced state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncStatusReport {
    /// ISO-8601 timestamp of the last observed sync, if any.
    pub last_sync: Option<String>,

    /// Number of sync operations observed.
    pub sync_count: u64,

    /// Number of writes observed that are not yet known to be anchored or
    /// backed up (adapter-defined precision).
    pub pending_changes: u64,

    /// Cumulative transient sync errors observed.
    pub sync_errors: u64,

    /// Whether the adapter considers the sync path healthy right now.
    pub is_healthy: bool,
}

/// A downloadable, hash-attested backup of a user's database.
#[derive(Debug, Clone)]
pub struct BackupArchive {
    /// Suggested filename (e.g. `{user_id}-{timestamp}.sql.gz`).
    pub filename: String,

    /// Archive bytes (compressed SQL or adapter-native format; the format
    /// is adapter-defined and MUST be documented by the implementation).
    pub bytes: Vec<u8>,

    /// BLAKE2-256 hex digest of `bytes` as attested at export time; callers
    /// MUST anchor this digest via [`UserDbAdapter::stage_anchor`] if the
    /// archive is committed to durable storage.
    pub sha256: String,
}

/// An intent to anchor a write on the blockchain.
///
/// This is the HASH stage's commit hook: the adapter MUST persist the
/// intent atomically with the write it attests (same transaction where the
/// storage allows), so a crash can never leave data committed without a
/// recoverable anchor intent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnchorIntent {
    /// User whose write is being anchored.
    pub user_id: String,

    /// Logical stream the anchor belongs to (adapter-defined namespace, e.g.
    /// `data_approval`, `vault_snapshot`, `backup`).
    pub stream: String,

    /// BLAKE2-256 hex fingerprint of the payload being anchored.
    pub fingerprint: String,

    /// Opaque payload reference (row id, URI, serialized fragment) the
    /// anchor consumer uses to resolve what was hashed. Never the payload
    /// itself — hashes anchor fingerprints, not bulk data.
    pub payload_ref: Value,
}

/// Lifecycle status of an [`AnchorIntent`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AnchorStatus {
    /// Recorded, not yet anchored. **Quarantined**: consumers MUST treat the
    /// attested data as not-final — excluded from report/share/export paths
    /// until anchored or reconciled.
    Pending,

    /// Anchored on-chain with a block hash. Data is final.
    Anchored,

    /// Exhausted the bounded retry budget without anchoring. Quarantined
    /// like `Pending`; requires reconciliation (repair), not silent final.
    Failed,
}

/// Minimal raw-SQL escape hatch.
///
/// Deliberately tiny: parameterized statement execution with JSON-typed
/// parameters and rows. Implementations bridge to their driver (libsql,
/// sqlite, …); the protocol does not grow this into an ORM.
#[async_trait]
pub trait SqlConnection: Send + Sync {
    /// Execute a statement, returning rows affected.
    async fn execute(&self, sql: &str, params: Vec<Value>) -> Result<u64, AdapterError>;

    /// Run a query, returning result rows as JSON objects.
    async fn query(&self, sql: &str, params: Vec<Value>) -> Result<Vec<Value>, AdapterError>;
}

/// A handle to one user database, obtained from [`UserDbAdapter::open`].
#[async_trait]
pub trait SqlDatabase: Send + Sync + Debug {
    /// Open a connection on this database.
    async fn connect(&self) -> Result<Box<dyn SqlConnection>, AdapterError>;
}

/// Stage-3/4 protocol trait: per-user database lifecycle + hash-commit hook.
///
/// The user is the single writer of their database; the only follower is
/// their encrypted backup. Implementations must document how single-writer
/// ownership is enforced (lease/fencing) if the deployment has any server
/// path that writes on the user's behalf.
#[async_trait]
pub trait UserDbAdapter: Send + Sync + Debug {
    /// Provision the user's database (create-if-absent), including any
    /// required schema initialization and key-envelope creation. Idempotent
    /// and safe under concurrent first use.
    async fn provision(&self, user_id: &str) -> Result<ProvisionedDatabase, AdapterError>;

    /// Pure naming: the logical database name/url for `user_id`. No I/O.
    fn database_url(&self, user_id: &str) -> String;

    /// Whether cloud sync/backup is configured for this deployment.
    fn is_cloud_available(&self) -> bool;

    /// Resolve short-lived connection material for the user's remote
    /// database. Implementations own token minting and caching; the returned
    /// material MUST be scoped to exactly this user and database.
    async fn connection_info(&self, user_id: &str) -> Result<ConnectionInfo, AdapterError>;

    /// Open the user's remote database.
    async fn open(&self, user_id: &str) -> Result<Box<dyn SqlDatabase>, AdapterError>;

    /// Liveness/readiness of the user's sync/backup path.
    async fn sync_status(&self, user_id: &str) -> Result<SyncStatusReport, AdapterError>;

    /// Resolve the user's encryption material (envelope-first with legacy
    /// fallback, rotation/migration per the implementation's envelope
    /// versions). Fail closed on corrupt key material.
    async fn resolve_encryption_key(
        &self,
        conn: &dyn SqlConnection,
        user_id: &str,
    ) -> Result<EncryptionKey, AdapterError>;

    /// Export a downloadable, hash-attested backup of the user's database.
    ///
    /// Greenfield in the reference deployment (the per-user cloud vault is
    /// the backup there); implementations that cannot export MUST return
    /// [`AdapterError::Unsupported`].
    async fn backup_export(&self, user_id: &str) -> Result<BackupArchive, AdapterError>;

    /// Stage an anchor intent **atomically with the write it attests**.
    ///
    /// This is the protocol's hardest rule (no data final without a hash):
    /// the intent lands in the same durable transaction as the data write
    /// wherever the storage allows, so crash recovery can always re-drive
    /// anchoring for rows left in [`AnchorStatus::Pending`].
    async fn stage_anchor(&self, intent: &AnchorIntent) -> Result<(), AdapterError>;

    /// Fetch the current lifecycle status of a previously staged anchor.
    async fn anchor_status(
        &self,
        conn: &dyn SqlConnection,
        user_id: &str,
        stream: &str,
        fingerprint: &str,
    ) -> Result<Option<AnchorStatus>, AdapterError>;
}
