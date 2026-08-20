# FreshCredit Auth Crate

**Version:** 0.1.0  
**Status:** Active  
**Purpose:** Shared authentication and authorization for FreshCredit platform

---

## Overview

This crate provides unified authentication functionality for all FreshCredit services, consolidating previously duplicated auth code from `apps/api`, `apps/web`, and `apps/adapters`.

## Features

| Feature | Description | Default |
|---------|-------------|---------|
| `jwt` | JWT token generation and validation | ✅ Yes |
| `oauth` | OAuth 2.0 / OIDC client support | No |
| `rbac` | Role-based access control | ✅ Yes |
| `session` | Session management integration | No |
| `biometric` | WebAuthn/biometric authentication | No |
| `axum` | Axum extractors and middleware | No |

## Quick Start

```rust
use freshcredit_auth::{AuthenticatedUser, jwt::JwtManager};

// Create JWT manager
let jwt_manager = JwtManager::new("your-secret-key-min-32-bytes!");

// Generate token
let token = jwt_manager.generate_token(
    "user-123",
    "user@example.com",
    "John Doe",
    "entra-id-456",
    3600, // expires in 1 hour
).unwrap();

// Validate token
let claims = jwt_manager.validate_token(&token).unwrap();
```

## Architecture

```
┌─────────────────────────────────────────┐
│         Application Layer               │
│  (apps/api, apps/web, apps/adapters)   │
└─────────────────────────────────────────┘
                   │
                   ▼
┌─────────────────────────────────────────┐
│        freshcredit-auth                 │
│  ┌─────────────────────────────────┐    │
│  │  AuthExtractor                  │    │
│  │  JwtManager                     │    │
│  │  middleware::require_auth       │    │
│  └─────────────────────────────────┘    │
└─────────────────────────────────────────┘
```

## Modules

- `types` - Core types: `AuthenticatedUser`, `AuthError`, `JwtClaims`
- `jwt` - JWT token management
- `extractors` - Axum extractors for authenticated requests
- `middleware` - Auth middleware for Axum
- `rbac` - Role-based access control
- `oauth` - OAuth/OIDC integration
- `session` - Session management

## Migration from App-Specific Auth

### Before (in apps/api/src/middleware/auth.rs)
```rust
pub struct AuthenticatedUser { /* duplicated */ }
pub struct AuthExtractor { /* duplicated */ }
```

### After
```rust
use freshcredit_auth::{AuthenticatedUser, AuthExtractor};
```

## Security

- JWT tokens use HS256 signing
- Minimum secret key length: 32 bytes
- Default token expiration: 1 hour
- Default refresh token expiration: 7 days

## License

MIT OR Apache-2.0
