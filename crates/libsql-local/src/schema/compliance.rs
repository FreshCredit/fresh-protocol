//! Compliance monitoring schema definitions
//!
//! Contains compliance tables:
//! - compliance_scans: Scan records
//! - compliance_rules: Rule definitions
//! - compliance_findings: Scan findings
//! - compliance_evidence: Evidence for findings
//!
//! COMPLIANCE: §14 Compliance Monitoring System
//! COMPLIANCE: §10 Unified Database Schema Architecture

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

    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_compliance_module_compiles() {
        let _ = 1 + 1; // Compile-time verification
    }
}

