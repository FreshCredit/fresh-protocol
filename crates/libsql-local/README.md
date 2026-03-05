---
title: "freshcredit-libsql-local"
date: "2026-03-05"
last_reviewed: "2026-03-05"
status: "stable"
category: "local"
---
# freshcredit-libsql-local

<!-- DOC-002: Stable -->
<!-- DOC-007: Active -->
<!-- Local libSQL database for FreshCredit -->
<!-- Last Updated: 2026-03-05 -->

Local libSQL database client for development and testing.

---

## Overview

Provides local SQLite-compatible database:
- Development database
- Test isolation
- Embedded mode

---

## Usage

```rust
use freshcredit_libsql_local::LocalDatabase;

let db = LocalDatabase::new("./data/local.db").await?;
```

---

## Environment Variables

```bash
DATABASE_URL=libsql://localhost:8080
DATABASE_AUTH_TOKEN=your_token
```

---

*Last Updated: 2026-03-05*
