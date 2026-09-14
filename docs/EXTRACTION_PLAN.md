# Protocol Extraction Plan (from FreshCredit/beta)

Status: APPROVED PLAN, Phase 1 · Decision recorded 2026-09-12

## Decision: `git filter-repo` over graduated copy

The products repo (`FreshCredit/beta`) has a young, shallow history (342
commits since 2026-08-20, ~470 MB pack). Filtering it per crate group is
cheap and preserves per-crate history, blame, and the LICENSE lineage.
Graduated copy (file-copy without history) is the fallback only if
filter-repo output is unmanageable.

## Crate groups (in extraction order — leaves first)

Each group = one PR in `fresh-protocol` + one cutover PR in `FreshCredit/beta`.

1. **Spec kernel** — `freshcredit-protocol-spec` (already designed in the
   products repo, PR #916; moves here first — it has zero internal deps).
2. **Core value types** — `crates/core/types`, `crates/core/security`,
   `crates/core/timing` (minus product config).
3. **DB layer** — `crates/db/libsql/common`, `local`, `cloud` (adapter impls
   land here per D2: protocol keeps trait + reference impls, products keep
   vendor-specific glue), `db/migrations` (protocol subset only — staging
   tables are product review-flow, D1: **staging stays in products**).
4. **Consistency kernel** — `crates/consistency` (leases, fencing, outbox).
5. **Auth + middleware** — `crates/auth`, `crates/middleware/{session,
   consent, idempotency, rate_limit}`.
6. **Anchor client** — `crates/blockchain-client` + hash-commit hooks.
7. **Ports** — `crates/engine-ports` (~90 port modules; prune product-shaped
   ports during the move).

   **Scope decision (2026-09-14): NOT MOVED — stays product-side.** Pruning
   during extraction revealed the crate is product-shaped at its core:
   `domain/` (User, Payment, Report — ~60% of all workspace consumption,
   1,050 use sites) is the FreshCredit credit/commerce domain by
   definition; the remaining modules are precisely the network/service
   integration traits (plaid, dnb, experian, salesforce, …) that the
   architecture places in the product layer (org-supported integrations),
   not the edge protocol. The protocol's extension surface already exists
   in `fresh-protocol-spec` (`UserDbAdapter`, `IdentityProvider` /
   `DidIssuer` / `DidVerifier`). Splitting engine-ports would churn
   1,000+ imports across the products repo for zero consolidation gain.
   Recorded as a deliberate end state, per the plan's own rule that
   partial consolidation is valid.
8. **Node** — `apps/blockchain` (the Substrate node), NOMT stack (5 crates,
   smoldot-adjacent; GPL+Classpath note already in README).

## Mechanics per group

1. `git filter-repo --path <paths> --path-rename crates/:crates/` on a fresh
   clone of `FreshCredit/beta`; push to a `migrate/<group>` branch here.
2. In `fresh-protocol`: workspace wiring, `[workspace.package]`, license
   headers, `#![forbid(unsafe_code)]`, P10 gate wired in (port
   `scripts/quality/p10-scan` in group 1), README layout update.
3. Tag `v0.1.0-<group>` when green (fmt + strict clippy + tests), **and move
   the unified consumption tag `v0.1.0`** to the same commit.
4. In `FreshCredit/beta` (cutover PR): delete moved crates, add
   `{ crate = { git = "https://github.com/FreshCredit/fresh-protocol", tag =
   "v0.1.0" } }` to `[workspace.dependencies]`, re-point path deps,
   full gates + deploy-verify per standard discipline.

   **MUST: one tag for ALL protocol crates.** Group 3 (2026-09-12) learned
   this the hard way: per-group tags make cargo check out the repo once per
   tag, so cross-group in-repo path deps (`libsql-local` → `types`) resolve
   as *different packages* than the same-name crates consumed from another
   tag — `multiple_crate_versions` fires on identical `0.1.0, 0.1.0` and
   the two copies are type-incompatible. A single tag = a single checkout =
   in-repo path deps unify with the workspace consumption. Cargo.lock pins
   the exact SHA, so builds stay reproducible even though the tag moves.
5. hakari: regenerate after each cutover (`cargo hakari generate`).

## Sequencing rules

- A group moves only when every crate in it has zero remaining deps on
  product crates (check with `cargo tree -i -p <crate>`; the SIM-001
  zero-product-deps rule is the template).
- `workspace-hack` stays product-side; the protocol repo does not adopt
  hakari (small dep graph).
- No behavior changes during extraction — mechanical moves only; any
  necessary adaptation happens in a separate, reviewable PR.
- PII-bearing or provider-shaped code found inside candidate crates is
  flagged and stays behind (product-side) rather than moved and fixed.

## Open items at execution time

- Org transfer: repo currently lives at `FreshCredit/fresh-protocol`;
  transfer to the dedicated org when its GitHub org is created (owner
  decision, billing-related).
- Identity plugins (Entra re-implementation against `IdentityProvider`,
  KILT in-protocol) — after groups 1–5 land.
- DID-key-derived BYOK (D3) — after extraction completes.
