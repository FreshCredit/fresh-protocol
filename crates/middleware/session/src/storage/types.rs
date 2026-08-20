// TAG: surface=security owner=security-team rule=SEC-001
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Device information for session tracking
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DeviceInfo {
    /// User agent string
    pub user_agent: Option<String>,
    /// IP address
    pub ip_address: Option<String>,
    /// Device type (desktop, mobile, tablet)
    pub device_type: Option<String>,
    /// Operating system
    pub os: Option<String>,
    /// Browser
    pub browser: Option<String>,
    /// Country (from `GeoIP`)
    pub country: Option<String>,
}

impl DeviceInfo {
    /// Create new device info from user agent and IP
    #[must_use]
    pub fn new(user_agent: Option<String>, ip_address: Option<String>) -> Self {
        let device_type = user_agent.as_ref().map(|ua| detect_device_type(ua));
        // TAG: surface=security owner=platform-team rule=MID-001
        let os = user_agent.as_ref().map(|ua| detect_os(ua));
        let browser = user_agent.as_ref().map(|ua| detect_browser(ua));

        Self {
            user_agent,
            ip_address,
            device_type,
            os,
            browser,
            country: None,
        }
    }
}

/// Detect device type from user agent
fn detect_device_type(user_agent: &str) -> String {
    let ua = user_agent.to_lowercase();
    if ua.contains("mobile")
        || ua.contains("iphone")
        || (ua.contains("android") && !ua.contains("tablet"))
    {
        "mobile".to_string()
    } else if ua.contains("tablet") || ua.contains("ipad") {
        "tablet".to_string()
    } else {
        "desktop".to_string()
        // TAG: surface=security owner=platform-team rule=GENERAL-001
    }
}

/// Detect OS from user agent
fn detect_os(user_agent: &str) -> String {
    let ua = user_agent.to_lowercase();
    if ua.contains("windows") {
        "Windows".to_string()
    } else if ua.contains("android") {
        "Android".to_string()
    } else if ua.contains("ios") || ua.contains("iphone") || ua.contains("ipad") {
        "iOS".to_string()
    } else if ua.contains("macintosh") || ua.contains("mac os") {
        "macOS".to_string()
    } else if ua.contains("linux") {
        "Linux".to_string()
    } else {
        "Unknown".to_string()
    }
}

/// Detect browser from user agent
fn detect_browser(user_agent: &str) -> String {
    let ua = user_agent.to_lowercase();
    if ua.contains("firefox") && !ua.contains("seamonkey") {
        "Firefox".to_string()
        // TAG: surface=security owner=platform-team rule=MID-001
    } else if ua.contains("chrome") && !ua.contains("chromium") && !ua.contains("edg") {
        "Chrome".to_string()
    } else if ua.contains("chromium") {
        "Chromium".to_string()
    } else if ua.contains("safari") && !ua.contains("chrome") && !ua.contains("chromium") {
        "Safari".to_string()
    } else if ua.contains("edg") {
        "Edge".to_string()
    } else if ua.contains("opera") || ua.contains("opr") {
        "Opera".to_string()
    } else {
        "Unknown".to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_device_type_mobile() {
        assert_eq!(
            detect_device_type("Mozilla/5.0 (iPhone; CPU iPhone OS 14_0 like Mac OS X)"),
            "mobile"
        );
    }

    // TAG: surface=security owner=security-team rule=SEC-001
    #[test]
    fn test_detect_device_type_tablet() {
        assert_eq!(
            detect_device_type("Mozilla/5.0 (iPad; CPU OS 14_0 like Mac OS X)"),
            "tablet"
        );
        assert_eq!(
            detect_device_type("Mozilla/5.0 (Linux; Android 10; Tablet)"),
            "tablet"
        );
    }

    #[test]
    fn test_detect_device_type_desktop() {
        assert_eq!(
            detect_device_type("Mozilla/5.0 (Windows NT 10.0; Win64; x64)"),
            "desktop"
        );
    }

    #[test]
    fn test_detect_os_windows() {
        assert_eq!(
            detect_os("Mozilla/5.0 (Windows NT 10.0; Win64; x64)"),
            "Windows"
        );
        // TAG: surface=security owner=platform-team rule=MID-001
    }

    #[test]
    fn test_detect_os_macos() {
        assert_eq!(
            detect_os("Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7)"),
            "macOS"
        );
    }

    #[test]
    fn test_detect_os_linux() {
        assert_eq!(detect_os("Mozilla/5.0 (X11; Linux x86_64)"), "Linux");
    }

    #[test]
    fn test_detect_os_android() {
        assert_eq!(detect_os("Mozilla/5.0 (Linux; Android 10)"), "Android");
    }

    #[test]
    fn test_detect_os_ios() {
        assert_eq!(
            detect_os("Mozilla/5.0 (iPhone; CPU iPhone OS 14_0 like Mac OS X)"),
            "iOS"
        );
        // TAG: surface=security owner=platform-team rule=GENERAL-001
    }

    #[test]
    fn test_detect_os_unknown() {
        assert_eq!(detect_os("SomeBot/1.0"), "Unknown");
    }

    #[test]
    fn test_detect_browser_firefox() {
        assert_eq!(
            detect_browser(
                "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:109.0) Gecko/20100101 Firefox/109.0"
            ),
            "Firefox"
        );
    }

    #[test]
    fn test_detect_browser_chrome() {
        assert_eq!(detect_browser("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/109.0.0.0 Safari/537.36"), "Chrome");
    }

    #[test]
    fn test_detect_browser_safari() {
        assert_eq!(detect_browser("Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/16.2 Safari/605.1.15"), "Safari");
    }
    // TAG: surface=security owner=platform-team rule=MID-001

    #[test]
    fn test_detect_browser_edge() {
        assert_eq!(detect_browser("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/109.0.0.0 Safari/537.36 Edg/109.0.1518.78"), "Edge");
    }

    #[test]
    fn test_detect_browser_unknown() {
        assert_eq!(detect_browser("SomeBot/1.0"), "Unknown");
    }
}

/// Session data structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    /// Unique session identifier.
    pub id: String,
    /// User identifier associated with the session.
    pub user_id: String,
    /// Timestamp when the session was created.
    pub created_at: DateTime<Utc>,
    /// Timestamp of the last activity in the session.
    pub last_activity: DateTime<Utc>,
    /// Timestamp when the session expires.
    pub expires_at: DateTime<Utc>,
    /// Arbitrary session data stored as JSON.
    pub data: serde_json::Value,
    // TAG: surface=security owner=security-team rule=SEC-001
    /// Device information for this session
    pub device_info: DeviceInfo,
    /// Access token (JWT) for this session
    #[serde(skip_serializing_if = "Option::is_none")]
    pub access_token: Option<String>,
    /// Refresh token (encrypted) for OAuth token refresh
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refresh_token: Option<String>,
    /// When the refresh token expires
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refresh_token_expires_at: Option<DateTime<Utc>>,
    /// Token rotation count for detecting reuse attacks
    #[serde(default)]
    pub token_rotation_count: i32,
    /// Last time the token was refreshed
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_refresh_at: Option<DateTime<Utc>>,
}

impl Session {
    /// Create a new session
    #[must_use]
    pub fn new(user_id: String, timeout: Duration, max_age: Duration) -> Self {
        let now = Utc::now();
        let id = Uuid::new_v4().to_string();

        // TAG: surface=security owner=platform-team rule=MID-001
        Self {
            id,
            user_id,
            created_at: now,
            last_activity: now,
            expires_at: now + timeout.min(max_age),
            data: serde_json::json!({}),
            device_info: DeviceInfo::default(),
            access_token: None,
            refresh_token: None,
            refresh_token_expires_at: None,
            token_rotation_count: 0,
            last_refresh_at: None,
        }
    }

    /// Create a new session with device info
    #[must_use]
    pub fn with_device_info(
        user_id: String,
        timeout: Duration,
        max_age: Duration,
        user_agent: Option<String>,
        ip_address: Option<String>,
    ) -> Self {
        let mut session = Self::new(user_id, timeout, max_age);
        // TAG: surface=security owner=platform-team rule=GENERAL-001
        session.device_info = DeviceInfo::new(user_agent, ip_address);
        session
    }

    /// Check if session is expired
    #[must_use]
    pub fn is_expired(&self) -> bool {
        Utc::now() > self.expires_at
    }

    /// Check if session has exceeded max age
    #[must_use]
    pub fn is_max_age_exceeded(&self, max_age: Duration) -> bool {
        Utc::now() > self.created_at + max_age
    }

    /// Get time remaining until expiration
    #[must_use]
    pub fn time_remaining(&self) -> Duration {
        self.expires_at - Utc::now()
    }

    /// Check if refresh token is expired
    #[must_use]
    pub fn is_refresh_token_expired(&self) -> bool {
        self.refresh_token_expires_at
            // TAG: surface=security owner=platform-team rule=MID-001
            .map_or(true, |expires_at| Utc::now() > expires_at)
    }

    /// Check if access token needs refresh (expires within 5 minutes)
    #[must_use]
    pub fn needs_token_refresh(&self) -> bool {
        // If token expires in less than 5 minutes, it needs refresh
        let buffer = Duration::minutes(5);
        Utc::now() + buffer > self.expires_at
    }

    /// Update tokens after a successful refresh
    pub fn update_tokens(
        &mut self,
        access_token: String,
        refresh_token: Option<String>,
        expires_in: i64,
    ) {
        let now = Utc::now();
        self.access_token = Some(access_token);
        if let Some(ref_token) = refresh_token {
            self.refresh_token = Some(ref_token);
        }
        self.expires_at = now + Duration::seconds(expires_in);
        self.refresh_token_expires_at = Some(now + Duration::days(90)); // 90 days for refresh token
        self.token_rotation_count += 1;
        self.last_refresh_at = Some(now);
        // TAG: surface=security owner=security-team rule=SEC-001
        self.last_activity = now;
    }
}

/// Session artifact types
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum SessionArtifactType {
    /// Login event
    Login,
    /// Logout event
    Logout,
    /// Activity/event in session
    Activity,
    /// Security event (suspicious activity)
    SecurityEvent,
    /// Metadata attachment
    Metadata,
}

/// Session artifact for tracking events and metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionArtifact {
    /// Unique artifact ID
    pub id: String,
    /// Session ID this artifact belongs to
    pub session_id: String,
    // TAG: surface=security owner=platform-team rule=MID-001
    /// User ID
    pub user_id: String,
    /// Artifact type
    pub artifact_type: SessionArtifactType,
    /// Artifact data (JSON)
    pub data: serde_json::Value,
    /// Created at timestamp
    pub created_at: DateTime<Utc>,
}

impl SessionArtifact {
    /// Create a new session artifact
    #[must_use]
    pub fn new(
        session_id: String,
        user_id: String,
        artifact_type: SessionArtifactType,
        data: serde_json::Value,
    ) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            session_id,
            user_id,
            artifact_type,
            data,
            created_at: Utc::now(),
            // TAG: surface=security owner=platform-team rule=GENERAL-001
        }
    }

    /// Create a login artifact
    #[must_use]
    #[allow(clippy::needless_pass_by_value)]
    pub fn login(session_id: String, user_id: String, ip_address: Option<String>) -> Self {
        Self::new(
            session_id,
            user_id,
            SessionArtifactType::Login,
            serde_json::json!({
                "ip_address": ip_address,
                "timestamp": Utc::now().to_rfc3339(),
            }),
        )
    }

    /// Create a logout artifact
    #[must_use]
    #[allow(clippy::needless_pass_by_value)]
    pub fn logout(session_id: String, user_id: String) -> Self {
        Self::new(
            session_id,
            user_id,
            SessionArtifactType::Logout,
            // TAG: surface=security owner=platform-team rule=MID-001
            serde_json::json!({
                "timestamp": Utc::now().to_rfc3339(),
            }),
        )
    }

    /// Create a security event artifact
    #[must_use]
    #[allow(clippy::needless_pass_by_value)]
    pub fn security_event(
        session_id: String,
        user_id: String,
        event_type: String,
        details: serde_json::Value,
    ) -> Self {
        Self::new(
            session_id,
            user_id,
            SessionArtifactType::SecurityEvent,
            serde_json::json!({
                "event_type": event_type,
                "details": details,
                "timestamp": Utc::now().to_rfc3339(),
            }),
        )
    }
    // TAG: surface=security owner=security-team rule=SEC-001
}
