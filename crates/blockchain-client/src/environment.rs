//! Runtime environment detection for the blockchain client.
//!
//! Mirrors the reference implementation's `freshcredit-config` detection
//! (priority: `FRESHCREDIT_ENV` → Cloud Run `K_SERVICE` → GKE
//! `KUBERNETES_SERVICE_HOST` → `PORT=8080` → Local) without the
//! product-specific base-URL table.

/// Runtime environment the client is executing in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Environment {
    /// Local development (localhost, 127.0.0.1)
    Local,
    /// Staging / pre-production environment
    Staging,
    /// Production (Cloud Run, GKE)
    Production,
}

impl Environment {
    /// Automatically detect the runtime environment.
    /// Priority:
    /// 1. Explicit `FRESHCREDIT_ENV` environment variable
    /// 2. `K_SERVICE` (Cloud Run) or `KUBERNETES_SERVICE_HOST` (GKE) presence
    /// 3. `PORT` environment variable (Cloud Run sets this to 8080)
    /// 4. Default to [`Environment::Local`]
    #[must_use]
    pub fn detect() -> Self {
        use std::env;

        if let Ok(env_str) = env::var("FRESHCREDIT_ENV") {
            match env_str.to_lowercase().as_str() {
                "production" | "prod" => return Self::Production,
                "staging" | "stage" => return Self::Staging,
                "local" | "dev" | "development" => return Self::Local,
                _ => {}
            }
        }

        if env::var("K_SERVICE").is_ok() {
            return Self::Production;
        }

        if env::var("KUBERNETES_SERVICE_HOST").is_ok() {
            return Self::Production;
        }

        if let Ok(port) = env::var("PORT") {
            if port == "8080" && env::var("LOCAL_TESTING").is_err() {
                return Self::Production;
            }
        }

        Self::Local
    }

    /// Check if running in production.
    #[must_use]
    pub const fn is_production(&self) -> bool {
        matches!(self, Self::Production)
    }

    /// Check if running locally.
    #[must_use]
    pub const fn is_local(&self) -> bool {
        matches!(self, Self::Local)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Environment::detect reads process-wide env vars; these tests only cover
    // the pure predicate methods. Detection behavior is exercised via the
    // client's `from_env` integration tests in the products repo.
    #[test]
    fn production_is_production() {
        assert!(Environment::Production.is_production());
        assert!(!Environment::Production.is_local());
    }

    #[test]
    fn local_is_local() {
        assert!(Environment::Local.is_local());
        assert!(!Environment::Local.is_production());
    }

    #[test]
    fn staging_is_neither() {
        assert!(!Environment::Staging.is_production());
        assert!(!Environment::Staging.is_local());
    }
}
