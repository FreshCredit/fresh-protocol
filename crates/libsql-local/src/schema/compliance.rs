//! Compliance monitoring schema definitions
//!
//! Contains compliance tables:
//! - compliance_scans: Scan records
//! - compliance_rules: Rule definitions
//! - compliance_findings: Scan findings
//! - compliance_evidence: Evidence for findings
//! - audit_events: Enhanced audit trail for PII access and data classification
//!
//! COMPLIANCE: §14 Compliance Monitoring System
//! COMPLIANCE: §10 Unified Database Schema Architecture
//! COMPLIANCE: §5 Observability, Logging, and Audit Requirements

use anyhow::Result;
use libsql::Connection;

/// Initialize compliance monitoring tables
pub async fn initialize_compliance_tables(conn: &Connection) -> Result<()> {
    conn.execute(
        "CREATE TABLE IF NOT EXISTS compliance_scans (
            id TEXT PRIMARY KEY,
            scan_type TEXT NOT NULL,
            framework TEXT,
            status TEXT NOT NULL DEFAULT 'running',
            started_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            completed_at DATETIME,
            findings_count INTEGER DEFAULT 0,
            triggered_by TEXT
        )",
        (),
    )
    .await?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS compliance_rules (
            id TEXT PRIMARY KEY,
            framework TEXT NOT NULL,
            rule_code TEXT NOT NULL,
            title TEXT NOT NULL,
            description TEXT NOT NULL,
            severity TEXT NOT NULL,
            detection_pattern TEXT,
            remediation_template TEXT,
            is_active INTEGER NOT NULL DEFAULT 1,
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP
        )",
        (),
    )
    .await?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS compliance_findings (
            id TEXT PRIMARY KEY,
            scan_id TEXT NOT NULL,
            rule_id TEXT NOT NULL,
            severity TEXT NOT NULL,
            status TEXT NOT NULL DEFAULT 'open',
            file_path TEXT,
            line_number INTEGER,
            description TEXT NOT NULL,
            remediation_guidance TEXT,
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            resolved_at DATETIME,
            resolved_by TEXT,
            FOREIGN KEY (scan_id) REFERENCES compliance_scans (id) ON DELETE CASCADE,
            FOREIGN KEY (rule_id) REFERENCES compliance_rules (id) ON DELETE CASCADE,
            FOREIGN KEY (resolved_by) REFERENCES user_profile (id) ON DELETE SET NULL
        )",
        (),
    )
    .await?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS compliance_evidence (
            id TEXT PRIMARY KEY,
            finding_id TEXT NOT NULL,
            evidence_type TEXT NOT NULL,
            content TEXT NOT NULL,
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (finding_id) REFERENCES compliance_findings (id) ON DELETE CASCADE
        )",
        (),
    )
    .await?;

    // Enhanced audit events table for comprehensive data access tracking
    // Per Phase 2.4: Enhanced Audit Logging requirements
    conn.execute(
        "CREATE TABLE IF NOT EXISTS audit_events (
            id TEXT PRIMARY KEY,
            user_id TEXT NOT NULL,
            user_email TEXT,
            event_type TEXT NOT NULL,
            resource_type TEXT NOT NULL,
            resource_id TEXT,
            action TEXT NOT NULL,
            -- Data classification: public, internal, confidential, restricted
            data_classification TEXT NOT NULL DEFAULT 'internal' CHECK(data_classification IN ('public', 'internal', 'confidential', 'restricted')),
            -- JSON array of PII field names accessed
            pii_fields_accessed TEXT DEFAULT '[]',
            -- Justification required for admin access to restricted data
            access_justification TEXT,
            -- Hash of event data for integrity verification
            data_hash TEXT,
            -- Request metadata
            request_path TEXT,
            request_method TEXT,
            response_status INTEGER,
            ip_address TEXT,
            user_agent TEXT,
            -- Session and correlation
            session_id TEXT,
            correlation_id TEXT,
            -- Anomaly detection flags
            is_anomaly INTEGER DEFAULT 0,
            anomaly_reason TEXT,
            -- Timestamps
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (user_id) REFERENCES user_profile (id) ON DELETE CASCADE
        )",
        (),
    )
    .await?;

    // Create indexes for audit_events queries
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_audit_events_user_id ON audit_events(user_id)",
        (),
    )
    .await?;

    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_audit_events_event_type ON audit_events(event_type)",
        (),
    )
    .await?;

    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_audit_events_data_classification ON audit_events(data_classification)",
        (),
    )
    .await?;

    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_audit_events_created_at ON audit_events(created_at)",
        (),
    )
    .await?;

    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_audit_events_resource ON audit_events(resource_type, resource_id)",
        (),
    )
    .await?;

    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_audit_events_anomaly ON audit_events(is_anomaly) WHERE is_anomaly = 1",
        (),
    )
    .await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_compliance_module_compiles() {
        let _ = 1 + 1; // Compile-time verification
    }
}

