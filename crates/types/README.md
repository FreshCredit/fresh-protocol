---
title: "freshcredit-core-types"
date: "2026-03-05"
last_reviewed: "2026-03-05"
status: "stable"
category: "types"
---
# freshcredit-core-types

<!-- DOC-002: Stable -->
<!-- DOC-007: Active -->
<!-- Core domain types for FreshCredit platform -->
<!-- Last Updated: 2026-03-05 -->

Core domain types shared across the FreshCredit platform.

---

## Overview

This crate defines the fundamental domain types used throughout FreshCredit:
- User and account types
- Credit report data structures
- Transaction and payment types
- Audit log types

---

## Usage

```rust
use freshcredit_core_types::{User, Account, CreditReport};

let user = User::new("user@example.com");
let account = Account::for_user(&user);
```

---

## Key Types

| Type | Purpose |
|------|---------|
| `User` | Platform user entity |
| `Account` | Financial account |
| `CreditReport` | Credit report data |
| `Transaction` | Payment transaction |
| `AuditLog` | Audit trail entry |

---

*Last Updated: 2026-03-05*
