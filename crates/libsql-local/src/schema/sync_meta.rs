//! Version-vector sync metadata for per-user vaults (`CONSISTENCY_VERDICTS` #3/#5)
//!
//! The browser and the server `mirror_*` writers both write directly to the
//! same per-user vault primary, which serializes them. These tables give each
//! writer node a counter (`_sync_meta`) and record how far each consumer has
//! observed each producer (`_sync_seen`), so a vault-side transaction can
//! atomically compare-and-record concurrency: a write whose producer counter
//! is ahead of what the consumer last saw is concurrent, and is logged to
//! `sync_conflicts`. Resolution stays last-writer-wins (recorded, not yet
//! repaired — automated resolution is a later phase).
//!
//! All DDL is idempotent (`CREATE TABLE IF NOT EXISTS`) and applied lazily on
//! first mirror/push touch; existing vaults converge on first write and rows
//! are never backfilled (vectors start from the first detection-enabled write).
//!
//! COMPLIANCE: §10 Unified Database Schema Architecture

use anyhow::Result;
use libsql::Connection;

/// Node id of the server-side `mirror_*` vault writer.
pub const SERVER_MIRROR_NODE_ID: &str = "server:mirror";

/// Prefix every browser device node id carries (`browser:<device-id>`).
pub const BROWSER_NODE_PREFIX: &str = "browser:";

/// Ordering verdict for one producer/consumer counter pair.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyncWriteOrder {
    /// The producer advanced past what the consumer last saw: the two sides
    /// wrote concurrently and silent LWW would discard one side.
    Conflict,
    /// The producer's counter is at or below what the consumer observed:
    /// strictly ordered (or nothing to compare yet).
    Ordered,
}

/// Classify one producer counter against the consumer's last-seen counter.
///
/// Pure heuristic behind the vault-side compare-and-record: `None` means the
/// producer node has never written (its row bootstraps at counter 0), which
/// seeds `last_seen` without a conflict.
#[must_use]
pub const fn classify_sync_write(producer_counter: Option<i64>, last_seen: i64) -> SyncWriteOrder {
    match producer_counter {
        Some(counter) if counter > last_seen => SyncWriteOrder::Conflict,
        Some(_) | None => SyncWriteOrder::Ordered,
    }
}

/// DDL for the per-writer-node counter table, one row per writer node.
const SYNC_META_DDL: &str = "CREATE TABLE IF NOT EXISTS _sync_meta (
    node_id TEXT PRIMARY KEY,
    counter INTEGER NOT NULL DEFAULT 0
)";

/// DDL for the consumer→producer observation table.
const SYNC_SEEN_DDL: &str = "CREATE TABLE IF NOT EXISTS _sync_seen (
    consumer_node TEXT,
    producer_node TEXT,
    last_counter INTEGER NOT NULL,
    PRIMARY KEY (consumer_node, producer_node)
)";

/// DDL for the recorded-conflict ledger. Detection only: `resolution` stays
/// `lww_recorded` until automated resolution lands.
const SYNC_CONFLICTS_DDL: &str = "CREATE TABLE IF NOT EXISTS sync_conflicts (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    table_name TEXT,
    row_id TEXT,
    server_counter INTEGER,
    other_counter INTEGER,
    detected_at TEXT DEFAULT (unixepoch('subsec')),
    resolution TEXT NOT NULL DEFAULT 'lww_recorded'
)";

// TAG: surface=database owner=platform-team rule=DB-001
/// Lazily ensure the version-vector sync tables exist in a per-user vault.
///
/// Idempotent: safe to run on every mirror/push touch, including inside a
/// write transaction, so vaults provisioned before these tables existed
/// converge on first detection-enabled write.
///
/// # Errors
///
/// Returns an error if any DDL statement fails.
pub async fn ensure_sync_meta_tables(conn: &Connection) -> Result<()> {
    conn.execute(SYNC_META_DDL, ()).await?;
    conn.execute(SYNC_SEEN_DDL, ()).await?;
    conn.execute(SYNC_CONFLICTS_DDL, ()).await?;
    Ok(())
}
