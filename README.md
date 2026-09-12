# The Fresh Protocol

**Open-source edge-infrastructure protocol.** One user, one database, one hash at a time.

The Fresh Protocol is the minimal stack for building **secure edge infrastructure**: applications where each user is the single writer of their own data, holding one live copy in their local database, with one encrypted synced backup as the only follower — and every committed state anchored to a blockchain for verifiable, linearizable history.

It is developed and maintained by **The FreshCredit Org** (nonprofit) as open infrastructure, independent of any commercial product built on top of it. Commercial products (e.g. FreshCredit Inc.'s credit platform) consume this protocol as a versioned dependency; they are not part of it.

```
 AUTH  →  DID  →  DB  →  HASH
```

| Stage | Guarantee |
|---|---|
| **AUTH** | The user authenticates (pluggable identity providers; OIDC/OAuth2 today). |
| **DID** | A decentralized identifier is issued and verified for the authenticated user (pluggable DID issuers/verifiers — KILT ships in-protocol; Entra-style providers ship as external plugins against the same traits). The verified DID is the root of key material for database encryption and operation authorization. |
| **DB** | A per-user database is provisioned on first use (libSQL local-first; sync/backup behind the `UserDbAdapter` spec — bring your own adapter). The user is the **single writer**; the only follower is their encrypted backup. |
| **HASH** | Every committed state is anchored — a block hash attests the data. **No data is final without a hash.** Unanchored data is explicitly quarantined, never silently final. |

## What the protocol is

- **Edge infrastructure only.** Auth plumbing, DID issuance/verification, per-user database lifecycle, sync/backup adapter spec, blockchain anchoring, and the consistency machinery to make single-writer + one follower correct (leases, fencing, idempotency, outbox).
- **Deployable alone.** Any industry or user can deploy the protocol by itself — no vendor integration required.
- **The atomicity rule.** Polkadot/Substrate finality provides the atomic commit point; libSQL provides the data plane. With those two, no additional atomic-commit infrastructure is required.

## What the protocol is not

- **No network/service integrations.** Plaid, health data, USPTO, etc. are *product-layer* concerns built **on top** of the protocol, against its adapter traits.
- **No native app bridge.** The bridge is product-side.
- **No message broker required.** Coordination uses single-writer ownership + leases/fencing + the transactional outbox; linearizability across sync events is provided by blockchain finality ordering. (This is a documented design position, not an omission — see `docs/` as they land.)

## Repository layout

Extraction from the reference implementation (`FreshCredit/beta`) is underway per
[`docs/EXTRACTION_PLAN.md`](docs/EXTRACTION_PLAN.md). Present today:

```
crates/
  fresh-protocol-spec/        # the spec: UserDbAdapter + IdentityProvider /
                              # DidIssuer / DidVerifier (AUTH→DID→DB→HASH traits)
  types/                      # freshcredit-types — shared domain value types
  security/                   # freshcredit-security — encryption/token primitives
  timing/                     # freshcredit-core-timing — clocks, timeouts, settlement
  libsql-common/              # freshcredit-libsql-common — URLs, retry, connection factory
  libsql-local/               # freshcredit-libsql-local — per-user vault client + 55-table schema
  libsql-cloud/               # freshcredit-libsql-cloud — remote vault client
  db-migrations/              # freshcredit-db-migrations — ordered migration runner
scripts/quality/p10-scan/     # P10 quality gate (census + --gate, repo-local baseline)
```

Target structure as extraction completes:

```
crates/
  fresh-protocol-auth/        # AUTH stage: session, identity-provider plugin traits
  fresh-protocol-identity/    # DID stage: DidIssuer / DidVerifier traits, KILT plugin
  fresh-protocol-db/          # DB stage: UserDbAdapter reference impls, migrations
  fresh-protocol-anchor/      # HASH stage: anchoring client, hash-commit hooks, outbox
  fresh-protocol-consensus/   # leases, fencing tokens, idempotency, transactional outbox
  fresh-protocol-ports/       # shared port/adapter traits (~the extension surface)
```

## Licensing

Dual-licensed under **Apache-2.0** and **MIT** (`LICENSE-APACHE`, `LICENSE-MIT`) — choose either.

### Third-party component notes

- **smoldot** (light-client, used by the Polkadot anchoring path) is **GPL-3.0 with Classpath-exception-2.0**. Linking is compatible with our dual licensing and with proprietary downstream linking; the Classpath exception preserves that. It is feature-gated: the protocol compiles without the `smoldot` feature.
- The protocol contains **no `unsafe` code** in its own crates (enforced by `#![forbid(unsafe_code)]` and the P10 quality gate).

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md). All contributions are licensed under the same dual license. The `AUTH → DID → DB → HASH` invariant is the project's north star: changes that weaken it (e.g. committing user data without an anchor path, or anchoring without a quarantine rule for pending state) are architectural regressions and will not be merged.

## Code of conduct

Be kind, be rigorous, cite evidence. Safety-critical thinking (NASA Power-of-10 discipline) applies to every line.
