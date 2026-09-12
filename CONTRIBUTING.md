# Contributing to The Fresh Protocol

Thank you for contributing to open edge infrastructure. This document is the
whole contribution contract; if anything is unclear, open an issue before
writing code.

## Ground rules

1. **The invariant is `AUTH → DID → DB → HASH`.** Every change is reviewed
   against it:
   - no user data becomes *final* without an anchor (block hash) path;
   - unanchored (pending) state must be an explicit, queryable, quarantined
     state — never silently final;
   - the user remains the single writer of their own database; the only
     follower is their encrypted backup.
2. **Safe Rust only.** Protocol crates carry `#![forbid(unsafe_code)]`. No
   exceptions without a written, maintainer-approved justification.
3. **NASA Power-of-10 discipline.** The reference CI enforces the P10 quality
   gate (function size caps, recursion/loop triage whitelists, test-aware
   assertion ratchets). Keep them green.
4. **No new mandatory dependencies** without an issue arguing buy-vs-build
   (license, maintenance, cost). The protocol prefers small, well-licensed,
   open-source building blocks; copyleft *without* a linking exception is a
   hard no for the dependency graph.

## Development

```sh
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- \
  -D warnings -W clippy::pedantic -W clippy::nursery -W clippy::cargo
cargo test --workspace
```

## Pull requests

- One logical change per PR; keep diffs reviewable.
- New public traits and adapter specs require: rustdoc on every public item,
   a doc example or test demonstrating the intended use, and a note in the
   README if the change affects the `AUTH → DID → DB → HASH` story.
- Squash-merge is the default; maintainers may rebase.

## Governance

The protocol is maintained by **The FreshCredit Org** (nonprofit).
FreshCredit Lab LLC contributes engineering as the R&D partner; FreshCredit
Inc. consumes the protocol for its commercial products but holds no special
merge rights. Trademark questions (names, logos) go to the org, not to this
repo.

## License

By contributing, you agree your contribution is licensed under the project's
dual license: Apache-2.0 OR MIT, at the licensee's choice.
