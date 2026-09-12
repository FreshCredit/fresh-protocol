//! Stage-1/2 spec traits: pluggable identity (AUTH) and DID issuance /
//! verification (DID).

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::AdapterError;

/// Material an identity provider hands back after a successful
/// authentication exchange (code/token swap, OIDC callback, wallet
/// challenge, …).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthenticationResult {
    /// Provider-scoped subject identifier (e.g. an Entra `oid`, an OIDC
    /// `sub`). Deployments MUST treat this as the join key into their own
    /// user registry; the protocol never interprets it.
    pub subject: String,

    /// Raw claims returned by the provider, passed through verbatim for
    /// audit. Implementations SHOULD redact secrets before storing.
    pub claims: Value,
}

/// What the protocol needs to know about an authentication attempt.
#[derive(Debug, Clone)]
pub struct AuthenticationContext {
    /// The provider's authorization code / token / challenge material.
    pub code: String,

    /// PKCE verifier, if the flow used PKCE.
    pub pkce_verifier: Option<String>,

    /// Where the provider should send the user back (validated against the
    /// deployment's allow-list by the implementation).
    pub redirect_uri: Option<String>,
}

/// Stage 1 — AUTH: a pluggable identity provider.
///
/// Implementations perform the server-side half of the provider's login
/// flow and return a normalized subject. The KILT plugin ships in-protocol;
/// enterprise providers (Entra-style CIAM) ship as external plugins against
/// this same trait.
#[async_trait]
pub trait IdentityProvider: Send + Sync + std::fmt::Debug {
    /// Stable provider identifier used in configuration and audit logs
    /// (e.g. `kilt`, `entra`, `google`).
    fn provider_id(&self) -> &'static str;

    /// Exchange the callback material for an authenticated identity.
    async fn exchange(
        &self,
        ctx: &AuthenticationContext,
    ) -> Result<AuthenticationResult, AdapterError>;
}

/// A minimal, serialized DID document view.
///
/// The protocol treats DIDs as opaque identifiers plus whatever the
/// deployment needs for key resolution; this shape is a serialization
/// envelope, not a W3C DID Document conformance layer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DidDocument {
    /// The DID itself (e.g. `did:kilt:…`).
    pub did: String,

    /// Public key material or a reference resolvable by the verifier,
    /// serialized (multibase/hex/JWK — implementation-defined).
    pub public_key: Value,

    /// Proof-of-control material for the issuance request, if required.
    pub proof: Option<Value>,
}

/// Stage 2 — DID issuance: turns an authenticated subject into a DID.
#[async_trait]
pub trait DidIssuer: Send + Sync + std::fmt::Debug {
    /// Stable issuer identifier (e.g. `kilt`, `entra-vc`).
    fn issuer_id(&self) -> &'static str;

    /// Issue a DID (and/or verifiable credential bound to it) for an
    /// authenticated subject. The issued DID roots the key material used for
    /// database encryption and operation authorization downstream.
    async fn issue(&self, subject: &AuthenticationResult) -> Result<DidDocument, AdapterError>;
}

/// The result of verifying a DID's validity and control.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationResult {
    /// Whether the DID is valid and currently controlled by the presenter.
    pub valid: bool,

    /// Optional human/audit-readable reason when `valid` is `false`.
    pub reason: Option<String>,

    /// Revoked/expired signals surfaced by the verifier.
    pub revoked: bool,
}

/// Stage 2 — DID verification: proves a DID is valid and controlled.
#[async_trait]
pub trait DidVerifier: Send + Sync + std::fmt::Debug {
    /// Verify a DID document/presentation, including revocation and expiry
    /// checks. MUST fail closed: any verification error means `valid: false`.
    async fn verify(&self, did: &DidDocument) -> Result<VerificationResult, AdapterError>;
}
