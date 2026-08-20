# FreshCredit Middleware Crates

HTTP middleware crates for FreshCredit applications, providing cross-cutting concerns like session management, rate limiting, and consent handling.

## Structure

| Directory | Description |
|-----------|-------------|
| `consent/` | GDPR consent management middleware |
| `idempotency/` | Idempotency key handling for safe retries |
| `rate_limit/` | Rate limiting middleware |
| `session/` | Encrypted session management |

## Usage

Add to your `Cargo.toml`:

```toml
freshcredit-session = { path = "../../crates/middleware/session" }
freshcredit-rate-limit = { path = "../../crates/middleware/rate_limit" }
freshcredit-consent = { path = "../../crates/middleware/consent" }
freshcredit-idempotency = { path = "../../crates/middleware/idempotency" }
```

## Middleware Stack

Typical application middleware order:

1. **Rate Limiting** - Prevent abuse
2. **Session** - Establish user context
3. **Consent** - Check GDPR consent
4. **Idempotency** - Ensure safe retries

## Integration

Used by [`apps/web`](../../apps/web/) and [`apps/api`](../../apps/api/).
