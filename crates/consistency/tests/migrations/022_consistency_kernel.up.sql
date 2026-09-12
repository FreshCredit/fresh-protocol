-- Wave 2 consistency kernel: durable idempotency, job leases with fencing, outbox relay.
-- Applied to the platform database (not per-user vaults).

-- Durable idempotency records. `key` is the client-supplied idempotency key
-- scoped by `scope` (e.g. "webhooks.plaid", "decisions.create").
CREATE TABLE IF NOT EXISTS idempotency_records (
    key         TEXT PRIMARY KEY,
    scope       TEXT NOT NULL,
    status      TEXT NOT NULL DEFAULT 'in_flight'
                CHECK (status IN ('in_flight', 'completed')),
    response    TEXT,                      -- serialized response payload, set on completion
    created_at  INTEGER NOT NULL DEFAULT (unixepoch()),  -- seconds
    completed_at INTEGER
);

-- Expire stale in-flight claims so crashed handlers do not wedge a key forever.
CREATE INDEX IF NOT EXISTS idx_idempotency_records_status_created
    ON idempotency_records (status, created_at);

-- Job leases with fencing tokens. `epoch` is a monotonically increasing
-- fencing token: each new holder gets epoch = old epoch + 1, so stale
-- holders can be detected by any writer comparing epochs.
CREATE TABLE IF NOT EXISTS job_leases (
    name        TEXT PRIMARY KEY,           -- logical job name, e.g. "plaid.webhook.drain"
    epoch       INTEGER NOT NULL DEFAULT 0, -- fencing token, incremented on each claim
    holder      TEXT NOT NULL,              -- unique instance id of the current holder
    expires_at  INTEGER NOT NULL            -- unixepoch seconds — lease is void past this
);

-- Transactional outbox. Producers INSERT in the same transaction as the
-- state change; the relay publishes unpublished rows and stamps
-- published_at.
CREATE TABLE IF NOT EXISTS outbox (
    id           INTEGER PRIMARY KEY AUTOINCREMENT,
    topic        TEXT NOT NULL,             -- e.g. "decision.finalized", "blockchain.anchor"
    payload      TEXT NOT NULL,             -- JSON payload
    created_at   INTEGER NOT NULL DEFAULT (unixepoch()),
    published_at INTEGER                    -- NULL until successfully published
);

CREATE INDEX IF NOT EXISTS idx_outbox_unpublished
    ON outbox (published_at) WHERE published_at IS NULL;
