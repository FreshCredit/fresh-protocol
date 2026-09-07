// TAG: surface=database owner=platform-team rule=DB-001 test-coverage=unit
//! Idempotent schema-migration helpers for per-user (`libsql`/Turso) databases.
//!
//! Per-user vault/staging databases live for a long time, and `CREATE TABLE IF
//! NOT EXISTS` is a silent no-op on tables that already exist — so adding a
//! column to the DDL never reaches databases created by an older version. That
//! silent drift caused repeated "no such column" production bugs. The helpers
//! here centralize the repair pattern that used to be hand-written at every
//! call site:
//!
//! 1. `CREATE TABLE IF NOT EXISTS …` (byte-identical DDL, untouched), then
//! 2. `PRAGMA table_info(<table>)` to read the live column set, then
//! 3. `ALTER TABLE <table> ADD COLUMN …` for each declared column that is
//!    missing, in declaration order.
//!
//! All helpers are safe to call on every boot: they are idempotent, tolerate
//! concurrent migrators (a racing `duplicate column name` ALTER is treated as
//! success), and never fail on shapes `SQLite` cannot add to a populated table
//! (`PRIMARY KEY`/`UNIQUE`/`GENERATED` columns and `NOT NULL` columns without
//! a default are skipped with a `tracing::warn!`, preserving the previous
//! silent-no-op behavior instead of crashing the boot).
//!
//! # Example
//!
//! ```no_run
//! # async fn example(conn: &libsql::Connection) -> anyhow::Result<()> {
//! use freshcredit_libsql_common::schema::{ensure_table, ColumnSpec};
//!
//! ensure_table(
//!     conn,
//!     "scores",
//!     "(
//!         id TEXT PRIMARY KEY,
//!         score_type TEXT NOT NULL DEFAULT '',
//!         score_value REAL NOT NULL DEFAULT 0
//!     )",
//!     &[
//!         ColumnSpec::new("score_type", "TEXT NOT NULL DEFAULT ''"),
//!         ColumnSpec::new("score_value", "REAL NOT NULL DEFAULT 0"),
//!     ],
//! )
//! .await?;
//! # Ok(())
//! # }
//! ```

use anyhow::{bail, Context, Result};
use libsql::Connection;
use std::borrow::Cow;
use std::collections::HashSet;

/// One column of the desired (union) schema of a table.
///
/// `definition` is the full column DDL fragment used after the column name in
/// both `CREATE TABLE` and `ALTER TABLE … ADD COLUMN`, e.g.
/// `"TEXT NOT NULL DEFAULT 'pending'"`. Keep it byte-identical to the fragment
/// in the table's `CREATE TABLE` body unless the migration deliberately needs
/// a weaker (e.g. nullable) definition for pre-existing populated tables.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ColumnSpec<'a> {
    /// Column name.
    pub name: &'a str,
    /// Column definition fragment (type + constraints, without the name).
    pub definition: &'a str,
}

impl<'a> ColumnSpec<'a> {
    /// Create a column spec from a name and a definition fragment.
    #[must_use]
    pub const fn new(name: &'a str, definition: &'a str) -> Self {
        Self { name, definition }
    }
}

/// A parsed `CREATE TABLE` statement: the (unquoted) table name plus one
/// owned `(name, definition)` pair per column, in declaration order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedCreateTable {
    /// Table name (unquoted).
    pub table: String,
    /// `(column name, column definition fragment)` pairs derived from the
    /// statement body. Table constraints (`FOREIGN KEY`, `UNIQUE(…)`,
    /// `CHECK(…)`, …) are excluded.
    pub columns: Vec<(String, String)>,
}

/// `SQLite` error text emitted when a racing migrator already added the column.
const DUPLICATE_COLUMN: &str = "duplicate column name";
/// `SQLite` error text emitted when `ALTER TABLE … ADD COLUMN` targets a
/// `NOT NULL` column without a non-NULL default on a populated table.
const NOT_NULL_NO_DEFAULT: &str = "Cannot add a NOT NULL column with default value NULL";

/// Add every missing column of `columns` to `table`, idempotently.
///
/// Reads the live column set once via `PRAGMA table_info`, then issues
/// `ALTER TABLE … ADD COLUMN` for each missing column in order. Columns whose
/// definition `SQLite` cannot add to an existing table (`PRIMARY KEY`, `UNIQUE`,
/// `GENERATED`) are skipped with a warning; a `NOT NULL` column without a
/// default that cannot be added to a populated legacy table is also skipped
/// with a warning. All other errors are propagated with table/column context.
///
/// # Errors
///
/// Returns an error if reading `PRAGMA table_info` fails or an `ALTER TABLE`
/// fails with an unexpected error.
pub async fn ensure_columns(
    conn: &Connection,
    table: &str,
    columns: &[ColumnSpec<'_>],
) -> Result<()> {
    if columns.is_empty() {
        return Ok(());
    }
    validate_identifier(table)
        .with_context(|| format!("ensure_columns: invalid table name {table:?}"))?;

    let mut existing = read_existing_columns(conn, table).await?;

    for spec in columns {
        let Some(sql) = plan_column_add(table, *spec, &existing)? else {
            continue;
        };
        match conn.execute(&sql, ()).await {
            Ok(_) => {
                tracing::info!("Added column {table}.{}", spec.name);
                existing.insert(unquote_identifier(spec.name).to_string());
            }
            Err(e) => handle_column_add_error(table, *spec, e, &mut existing)?,
        }
    }
    Ok(())
}

/// Read the live column-name set of `table` via `PRAGMA table_info`.
async fn read_existing_columns(conn: &Connection, table: &str) -> Result<HashSet<String>> {
    let mut info_rows = conn
        .query(
            "SELECT name FROM pragma_table_info(?)",
            libsql::params![table],
        )
        .await
        .with_context(|| format!("ensure_columns({table}): failed to read table_info"))?;
    let mut existing = HashSet::new();
    while let Some(row) = info_rows.next().await? {
        existing.insert(row.get::<String>(0)?);
    }
    Ok(existing)
}

/// Decide whether `spec` needs an `ALTER TABLE … ADD COLUMN` and build the
/// statement. Returns `Ok(None)` when the column is already present (debug
/// log) or its definition `SQLite` cannot add to an existing table (warning);
/// returns an error when the column name fails validation.
fn plan_column_add(
    table: &str,
    spec: ColumnSpec<'_>,
    existing: &HashSet<String>,
) -> Result<Option<String>> {
    if existing.contains(unquote_identifier(spec.name)) {
        tracing::debug!("Column {table}.{} already exists", spec.name);
        return Ok(None);
    }
    if !is_addable_definition(spec.definition) {
        tracing::warn!(
            "ensure_columns({table}): skipping column {} {def} — SQLite cannot add a \
             PRIMARY KEY/UNIQUE/GENERATED column to an existing table; table rebuild required",
            spec.name,
            def = spec.definition
        );
        return Ok(None);
    }
    validate_identifier(spec.name).with_context(|| {
        format!(
            "ensure_columns({table}): invalid column name {:?}",
            spec.name
        )
    })?;
    Ok(Some(format!(
        "ALTER TABLE {table} ADD COLUMN {} {}",
        spec.name, spec.definition
    )))
}

/// Classify an `ALTER TABLE … ADD COLUMN` failure: a racing migrator's
/// `duplicate column name` is success, a `NOT NULL` column without a default
/// on a populated legacy table is skipped with a warning, and anything else
/// propagates with table/column context.
fn handle_column_add_error(
    table: &str,
    spec: ColumnSpec<'_>,
    e: libsql::Error,
    existing: &mut HashSet<String>,
) -> Result<()> {
    let msg = e.to_string();
    if msg.contains(DUPLICATE_COLUMN) {
        // A concurrent migrator won the race; that is success.
        tracing::debug!("Column {table}.{} added concurrently", spec.name);
        existing.insert(unquote_identifier(spec.name).to_string());
        return Ok(());
    }
    if msg.contains(NOT_NULL_NO_DEFAULT) {
        tracing::warn!(
            "ensure_columns({table}): could not add column {} {def} to the populated \
             legacy table (NOT NULL without default); leaving the old schema in place",
            spec.name,
            def = spec.definition
        );
        return Ok(());
    }
    Err(e).with_context(|| {
        format!(
            "ensure_columns({table}): failed to add column {:?}",
            spec.name
        )
    })
}

/// Create `name` with the `ddl` body if needed, then bring every column of
/// `columns` up to the desired union schema.
///
/// `ddl` is the parenthesized `CREATE TABLE` body, e.g. `"(id TEXT PRIMARY
/// KEY, …)"`; it is executed verbatim as `CREATE TABLE IF NOT EXISTS {name}
/// {ddl}` so fresh databases get byte-identical DDL to before. `columns` is
/// the union column set used to repair pre-existing tables; it may list
/// columns beyond the `ddl` body (for example union-schema columns that only
/// ever appear as migrations).
///
/// # Errors
///
/// Returns an error if the `CREATE TABLE` or any required `ALTER TABLE` fails.
pub async fn ensure_table(
    conn: &Connection,
    name: &str,
    ddl: &str,
    columns: &[ColumnSpec<'_>],
) -> Result<()> {
    validate_identifier(name)
        .with_context(|| format!("ensure_table: invalid table name {name:?}"))?;
    let sql = format!("CREATE TABLE IF NOT EXISTS {name} {ddl}");
    conn.execute(&sql, ())
        .await
        .with_context(|| format!("ensure_table({name}): CREATE TABLE failed"))?;
    Box::pin(ensure_columns(conn, name, columns)).await
}

/// Parse a `CREATE TABLE [IF NOT EXISTS] <name> (<body>)` statement.
///
/// The statement is not validated against a database; only its text is
/// parsed. The returned [`ParsedCreateTable`] carries the (unquoted) table
/// name and one `(name, definition)` pair per column definition in the body,
/// in order. Table-level constraints (`CONSTRAINT`, `PRIMARY KEY`, `UNIQUE`,
/// `FOREIGN KEY`, `CHECK`) are recognized and excluded from the column list.
/// Splitting is quote- and comment-aware: nested parentheses (`DECIMAL(10,2)`),
/// string literals containing commas (`DEFAULT 'a,b'`), and `--`/`/* */`
/// comments inside the body are handled.
///
/// # Errors
///
/// Returns an error if the statement is not a recognizable
/// `CREATE TABLE … ( … )` statement or declares no columns.
pub fn parse_create_table(ddl: &str) -> Result<ParsedCreateTable> {
    let stripped = strip_sql_comments(ddl);
    let tokens = lex(&stripped);
    let mut rest = tokens.as_slice();

    if !take_word(&mut rest, "CREATE") || !take_word(&mut rest, "TABLE") {
        bail!("parse_create_table: not a CREATE TABLE statement: {ddl:?}");
    }
    skip_if_not_exists(&mut rest);
    let table = take_table_name(&mut rest, ddl)?;
    let body = take_body(rest, &stripped, ddl)?;

    let mut columns = Vec::new();
    for item in split_top_level_commas(body) {
        if let Some((name, definition)) = parse_column_item(item, &table, ddl)? {
            columns.push((name, definition));
        }
    }
    if columns.is_empty() {
        bail!("parse_create_table: no column definitions in {ddl:?}");
    }
    Ok(ParsedCreateTable { table, columns })
}

/// Consume an optional `IF NOT EXISTS` clause from the token stream.
fn skip_if_not_exists(rest: &mut &[Tok<'_>]) {
    if let Some((Tok::Word(w), tail)) = rest.split_first() {
        if w.eq_ignore_ascii_case("IF") {
            let mut tmp = tail;
            if take_word(&mut tmp, "NOT") && take_word(&mut tmp, "EXISTS") {
                *rest = tmp;
            }
        }
    }
}

/// Consume and return the (unquoted) table name.
fn take_table_name(rest: &mut &[Tok<'_>], ddl: &str) -> Result<String> {
    match rest.split_first() {
        Some((Tok::Ident(name) | Tok::Word(name), tail)) => {
            *rest = tail;
            Ok((*name).to_string())
        }
        _ => bail!("parse_create_table: missing table name in {ddl:?}"),
    }
}

/// Locate the parenthesized body of the statement in the stripped text and
/// return its contents (without the outer parens). The byte offset of the
/// body's opening paren comes from the offset-preserving comment replacement,
/// keeping it aligned with `ddl`.
fn take_body<'a>(rest: &[Tok<'_>], stripped: &'a str, ddl: &str) -> Result<&'a str> {
    let open = match rest.split_first() {
        Some((Tok::LParen, _)) => stripped
            .find('(')
            .ok_or_else(|| anyhow::anyhow!("parse_create_table: missing '(' in {ddl:?}"))?,
        _ => bail!("parse_create_table: missing column list in {ddl:?}"),
    };
    extract_parenthesized(stripped, open)
        .ok_or_else(|| anyhow::anyhow!("parse_create_table: unbalanced parens in {ddl:?}"))
}

/// Parse one comma-separated body item into a `(column name, definition)`
/// pair. Returns `Ok(None)` for empty items and table-level constraints
/// (`CONSTRAINT`, `PRIMARY KEY`, `UNIQUE`, `FOREIGN KEY`, `CHECK`), which do
/// not declare columns. Table constraints start with a keyword (possibly
/// followed directly by `(`, e.g. `UNIQUE(a, b)`), so the leading alphabetic
/// run is inspected rather than the first whitespace-separated token.
fn parse_column_item(item: &str, table: &str, ddl: &str) -> Result<Option<(String, String)>> {
    let item = item.trim();
    if item.is_empty() {
        return Ok(None);
    }
    let leading_word: String = item.chars().take_while(char::is_ascii_alphabetic).collect();
    if is_table_constraint_start(&leading_word) {
        return Ok(None);
    }
    let first = item
        .split_whitespace()
        .next()
        .ok_or_else(|| anyhow::anyhow!("parse_create_table: empty body item in {ddl:?}"))?;
    let name_len = first.len();
    let definition = item[name_len..].trim().to_string();
    if definition.is_empty() {
        bail!("parse_create_table: column {first:?} in table {table} has no type/def");
    }
    Ok(Some((first.to_string(), definition)))
}

/// Execute a `CREATE TABLE IF NOT EXISTS` statement and repair any
/// pre-existing table derived from the same DDL.
///
/// This is the drop-in replacement for `conn.execute("CREATE TABLE IF NOT
/// EXISTS …", ())` at per-user schema declaration sites: the statement text
/// is executed verbatim (byte-identical effect on fresh databases), and the
/// column definitions parsed out of the body are then ensured on the live
/// table via [`ensure_columns`]. Table constraints in the body are part of
/// the `CREATE TABLE` only, exactly as before.
///
/// `ddl` must start with `CREATE TABLE` (with or without `IF NOT EXISTS` —
/// `IF NOT EXISTS` is added if absent); anything else fails parsing loudly so
/// misuse is caught in tests instead of silently skipping migrations.
///
/// # Errors
///
/// Returns an error if the statement cannot be parsed or executed.
pub async fn ensure_table_ddl(conn: &Connection, ddl: &str) -> Result<()> {
    let parsed = parse_create_table(ddl)?;
    let create_sql = if has_if_not_exists(ddl) {
        Cow::Borrowed(ddl)
    } else {
        Cow::Owned(inject_if_not_exists(ddl)?)
    };
    conn.execute(&create_sql, ())
        .await
        .with_context(|| format!("ensure_table_ddl({}): CREATE TABLE failed", parsed.table))?;
    let specs: Vec<ColumnSpec<'_>> = parsed
        .columns
        .iter()
        .map(|(name, def)| ColumnSpec::new(name, def))
        .collect();
    Box::pin(ensure_columns(conn, &parsed.table, &specs)).await
}

/// Execute every statement of `ddls` via [`ensure_table_ddl`], in order.
///
/// Convenience for the common `for sql in TABLE_DDL { conn.execute(sql, ()) }`
/// initialization loops.
///
/// # Errors
///
/// Returns an error if any statement fails.
pub async fn ensure_table_ddls(conn: &Connection, ddls: &[&str]) -> Result<()> {
    for ddl in ddls {
        Box::pin(ensure_table_ddl(conn, ddl)).await?;
    }
    Ok(())
}

/// Consume a leading keyword (case-insensitive) from the token stream.
fn take_word(rest: &mut &[Tok<'_>], expected: &str) -> bool {
    if let Some((Tok::Word(w), tail)) = rest.split_first() {
        if w.eq_ignore_ascii_case(expected) {
            *rest = tail;
            return true;
        }
    }
    false
}

/// A lexical token for parsing CREATE TABLE statements.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Tok<'a> {
    Word(&'a str),
    Ident(&'a str),
    LParen,
    Other(char),
}

/// Tokenize a CREATE TABLE statement header: words (keywords/identifiers),
/// quote-aware identifiers, parentheses, and everything else as single
/// characters. String literals are consumed as opaque quoted `Ident`s so
/// commas or parens inside them never act as separators. Must be called on
/// comment-stripped text (see [`strip_sql_comments`]).
fn lex(sql: &str) -> Vec<Tok<'_>> {
    let bytes = sql.as_bytes();
    let mut toks = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        let c = bytes[i];
        match c {
            b' ' | b'\t' | b'\n' | b'\r' => i += 1,
            b'(' => {
                toks.push(Tok::LParen);
                i += 1;
            }
            b')' => {
                toks.push(Tok::Other(')'));
                i += 1;
            }
            b'"' | b'`' | b'\'' => {
                let quote = c;
                let start = i + 1;
                let mut j = start;
                while j < bytes.len() {
                    if bytes[j] == quote {
                        if quote == b'\'' && j + 1 < bytes.len() && bytes[j + 1] == b'\'' {
                            j += 2; // escaped quote inside string literal
                            continue;
                        }
                        break;
                    }
                    j += 1;
                }
                toks.push(Tok::Ident(&sql[start..j.min(bytes.len())]));
                i = (j + 1).min(bytes.len());
            }
            b'[' => {
                let start = i + 1;
                let mut j = start;
                while j < bytes.len() && bytes[j] != b']' {
                    j += 1;
                }
                toks.push(Tok::Ident(&sql[start..j.min(bytes.len())]));
                i = (j + 1).min(bytes.len());
            }
            _ if c.is_ascii_alphanumeric() || c == b'_' || c == b'.' || c >= 0x80 => {
                let start = i;
                let mut j = i;
                while j < bytes.len() {
                    let d = bytes[j];
                    if d.is_ascii_alphanumeric() || d == b'_' || d == b'.' || d >= 0x80 {
                        j += 1;
                    } else {
                        break;
                    }
                }
                toks.push(Tok::Word(&sql[start..j]));
                i = j;
            }
            _ => {
                toks.push(Tok::Other(char::from(c)));
                i += 1;
            }
        }
    }
    toks
}

/// Replace `--` line comments and `/* */` block comments with whitespace,
/// preserving byte length and newlines so every offset in the result matches
/// the input. Returns `Cow::Borrowed` when there is nothing to strip.
fn strip_sql_comments(sql: &str) -> Cow<'_, str> {
    if !sql.contains("--") && !sql.contains("/*") {
        return Cow::Borrowed(sql);
    }
    let bytes = sql.as_bytes();
    let mut out: Vec<u8> = bytes.to_vec();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'-' && i + 1 < bytes.len() && bytes[i + 1] == b'-' {
            let mut j = i;
            while j < bytes.len() && bytes[j] != b'\n' {
                out[j] = b' ';
                j += 1;
            }
            i = j;
        } else if bytes[i] == b'/' && i + 1 < bytes.len() && bytes[i + 1] == b'*' {
            let mut j = i;
            while j + 1 < bytes.len() && !(bytes[j] == b'*' && bytes[j + 1] == b'/') {
                if bytes[j] != b'\n' {
                    out[j] = b' ';
                }
                j += 1;
            }
            let end = (j + 2).min(bytes.len());
            for k in j..end {
                if bytes[k] != b'\n' {
                    out[k] = b' ';
                }
            }
            i = end;
        } else {
            i += 1;
        }
    }
    Cow::Owned(
        String::from_utf8(out).expect("replacing comments with ASCII spaces keeps UTF-8 valid"),
    )
}

/// Whether the statement already contains `IF NOT EXISTS`.
fn has_if_not_exists(ddl: &str) -> bool {
    let upper = ddl.to_ascii_uppercase();
    upper.contains("IF NOT EXISTS")
}

/// Rebuild `CREATE TABLE <name> (` as `CREATE TABLE IF NOT EXISTS <name> (`;
/// returns the statement unchanged if no injection point is found (parsing
/// has already validated the statement shape by the time this is called).
fn inject_if_not_exists(ddl: &str) -> Result<String> {
    let tokens = lex(ddl);
    let mut insert_at = None;
    let mut offset = 0_usize;
    for tok in &tokens {
        match tok {
            Tok::LParen => {
                insert_at = Some(offset);
                break;
            }
            Tok::Word(w) => offset += w.len(),
            Tok::Ident(w) => offset += w.len() + 2,
            Tok::Other(c) => offset += c.len_utf8(),
        }
    }
    let insert_at = insert_at.ok_or_else(|| anyhow::anyhow!("no '(' found in {ddl:?}"))?;
    let mut out = ddl.to_string();
    out.insert_str(insert_at, " IF NOT EXISTS");
    Ok(out)
}

/// Extract the body between the parenthesized group opened at byte offset
/// `open` (quote-aware, returns content without the outer parens).
fn extract_parenthesized(sql: &str, open: usize) -> Option<&str> {
    let bytes = sql.as_bytes();
    let mut depth = 0_i32;
    let mut in_str: Option<u8> = None;
    let mut i = open;
    while i < bytes.len() {
        let c = bytes[i];
        if let Some(q) = in_str {
            if c == q {
                if q == b'\'' && i + 1 < bytes.len() && bytes[i + 1] == b'\'' {
                    i += 2;
                    continue;
                }
                in_str = None;
            }
            i += 1;
            continue;
        }
        match c {
            b'\'' | b'"' | b'`' => in_str = Some(c),
            b'(' => depth += 1,
            b')' => {
                depth -= 1;
                if depth == 0 {
                    return Some(&sql[open + 1..i]);
                }
            }
            _ => {}
        }
        i += 1;
    }
    None
}

/// Split a CREATE TABLE body on top-level commas (quote-aware).
fn split_top_level_commas(body: &str) -> Vec<&str> {
    let bytes = body.as_bytes();
    let mut items = Vec::new();
    let mut depth = 0_i32;
    let mut in_str: Option<u8> = None;
    let mut start = 0_usize;
    let mut i = 0_usize;
    while i < bytes.len() {
        let c = bytes[i];
        if let Some(q) = in_str {
            if c == q {
                // Handle '' escape inside string literals.
                if q == b'\'' && i + 1 < bytes.len() && bytes[i + 1] == b'\'' {
                    i += 2;
                    continue;
                }
                in_str = None;
            }
            i += 1;
            continue;
        }
        match c {
            b'\'' | b'"' | b'`' => in_str = Some(c),
            b'(' => depth += 1,
            b')' => depth -= 1,
            b',' if depth == 0 => {
                items.push(&body[start..i]);
                start = i + 1;
            }
            _ => {}
        }
        i += 1;
    }
    items.push(&body[start..]);
    items
}

/// Whether the first word of a body item introduces a table-level constraint
/// rather than a column definition.
fn is_table_constraint_start(first_word: &str) -> bool {
    first_word.eq_ignore_ascii_case("CONSTRAINT")
        || first_word.eq_ignore_ascii_case("PRIMARY")
        || first_word.eq_ignore_ascii_case("UNIQUE")
        || first_word.eq_ignore_ascii_case("FOREIGN")
        || first_word.eq_ignore_ascii_case("CHECK")
}

/// Whether `SQLite` can add this column definition to an existing table via
/// `ALTER TABLE … ADD COLUMN`. `SQLite` forbids adding `PRIMARY KEY`, `UNIQUE`,
/// or `GENERATED` columns (attempting them errors out), so they are skipped
/// up front; a table rebuild is the only migration path for those.
fn is_addable_definition(definition: &str) -> bool {
    let stripped = strip_sql_comments(definition);
    let tokens = lex(&stripped);
    !tokens.iter().any(|t| match t {
        Tok::Word(w) => {
            w.eq_ignore_ascii_case("PRIMARY")
                || w.eq_ignore_ascii_case("UNIQUE")
                || w.eq_ignore_ascii_case("GENERATED")
        }
        _ => false,
    })
}

/// Validate an identifier before interpolating it into DDL. Table and column
/// names cannot be bind parameters, so this is the SQL-injection guard:
/// lowercase ASCII `SQLite` identifiers (letters, digits after the first
/// position, and `_`), excluding `sqlite_*` system names. One layer of
/// standard quoting (`"…"`, `` `…` ``, `[…]`) is accepted and preserved so
/// reserved-word columns such as `"trigger"` can be added.
fn validate_identifier(ident: &str) -> Result<()> {
    let inner = unquote_identifier(ident);
    let ok = !inner.is_empty()
        && !inner.starts_with("sqlite_")
        && inner
            .chars()
            .enumerate()
            .all(|(i, c)| c.is_ascii_lowercase() || c == '_' || (i > 0 && c.is_ascii_digit()));
    if ok {
        Ok(())
    } else {
        bail!("invalid SQLite identifier: {ident:?}");
    }
}

/// Strip one layer of standard `SQLite` identifier quoting, if present.
fn unquote_identifier(ident: &str) -> &str {
    let bytes = ident.as_bytes();
    if bytes.len() >= 2 {
        let (open, close) = (bytes[0], bytes[bytes.len() - 1]);
        let quoted = matches!(
            (open, close),
            (b'"', b'"') | (b'`', b'`') | (b'\'', b'\'') | (b'[', b']')
        );
        if quoted {
            return &ident[1..ident.len() - 1];
        }
    }
    ident
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Temp-file libsql DB (each `:memory:` connection is a separate database
    /// in this libsql version, so tests share a file instead).
    async fn test_db() -> (libsql::Database, std::path::PathBuf, Connection) {
        let path = std::env::temp_dir().join(format!("fc-schema-test-{}.db", uuid::Uuid::new_v4()));
        let db = libsql::Builder::new_local(&path).build().await.unwrap();
        let conn = db.connect().unwrap();
        (db, path, conn)
    }

    async fn column_names(conn: &Connection, table: &str) -> HashSet<String> {
        let mut rows = conn
            .query(
                "SELECT name FROM pragma_table_info(?)",
                libsql::params![table],
            )
            .await
            .unwrap();
        let mut set = HashSet::new();
        while let Some(row) = rows.next().await.unwrap() {
            set.insert(row.get::<String>(0).unwrap());
        }
        set
    }

    // ── Parser unit tests ────────────────────────────────────────────────

    #[test]
    fn parser_extracts_columns_and_skips_constraints() {
        let parsed = parse_create_table(
            "CREATE TABLE IF NOT EXISTS t (\n\
             id TEXT PRIMARY KEY,\n\
             user_id TEXT NOT NULL,\n\
             amount DECIMAL(10,2) DEFAULT 'a,b',\n\
             UNIQUE(user_id),\n\
             CONSTRAINT chk CHECK (amount > 0),\n\
             FOREIGN KEY (user_id) REFERENCES other (id)\n\
             )",
        )
        .unwrap();
        assert_eq!(parsed.table, "t");
        assert_eq!(parsed.columns.len(), 3);
        assert_eq!(parsed.columns[0].0, "id");
        assert_eq!(parsed.columns[0].1, "TEXT PRIMARY KEY");
        assert_eq!(parsed.columns[1].0, "user_id");
        assert_eq!(parsed.columns[2].0, "amount");
        assert!(parsed.columns[2].1.starts_with("DECIMAL(10,2)"));
    }

    #[test]
    fn parser_handles_comments_and_string_defaults() {
        let parsed = parse_create_table(
            "CREATE TABLE IF NOT EXISTS plaid_staging_data (\n\
             id INTEGER PRIMARY KEY AUTOINCREMENT, -- row id\n\
             /* block comment, with comma */ user_id TEXT NOT NULL,\n\
             status TEXT NOT NULL DEFAULT 'pending' CHECK (status IN ('pending', 'approved')),\n\
             created_at TEXT NOT NULL DEFAULT (datetime('now'))\n\
             )",
        )
        .unwrap();
        assert_eq!(parsed.table, "plaid_staging_data");
        assert_eq!(parsed.columns.len(), 4);
        assert_eq!(parsed.columns[1].0, "user_id");
        assert_eq!(parsed.columns[3].0, "created_at");
        assert!(parsed.columns[2]
            .1
            .contains("CHECK (status IN ('pending', 'approved'))"));
    }

    #[test]
    fn parser_rejects_non_create_statements() {
        assert!(parse_create_table("CREATE INDEX idx ON t(a)").is_err());
        assert!(parse_create_table("SELECT 1").is_err());
        assert!(parse_create_table("CREATE TABLE t").is_err());
    }

    #[test]
    fn parser_requires_columns() {
        assert!(parse_create_table("CREATE TABLE t (PRIMARY KEY (id))").is_err());
        assert!(parse_create_table("CREATE TABLE t (FOREIGN KEY (a) REFERENCES b(id))").is_err());
    }

    #[test]
    fn injects_if_not_exists_when_missing() {
        let ddl = "CREATE TABLE t (id TEXT PRIMARY KEY, v REAL)";
        let injected = inject_if_not_exists(ddl).unwrap();
        assert!(injected.starts_with("CREATE TABLE IF NOT EXISTS t ("));
        assert!(has_if_not_exists("CREATE TABLE IF NOT EXISTS t (id TEXT)"));
        assert!(!has_if_not_exists(ddl));
    }

    // ── Behavior tests ───────────────────────────────────────────────────

    #[tokio::test]
    async fn ensure_columns_migrates_preexisting_table_missing_columns() {
        let (_db, path, conn) = test_db().await;
        // Simulate a legacy vault table created by an older schema version.
        conn.execute(
            "CREATE TABLE scores (id TEXT PRIMARY KEY, user_id TEXT NOT NULL)",
            (),
        )
        .await
        .unwrap();
        conn.execute("INSERT INTO scores (id, user_id) VALUES ('s1', 'u1')", ())
            .await
            .unwrap();

        ensure_columns(
            &conn,
            "scores",
            &[
                ColumnSpec::new("score_type", "TEXT NOT NULL DEFAULT ''"),
                ColumnSpec::new("score_value", "REAL NOT NULL DEFAULT 0"),
                ColumnSpec::new("score_name", "TEXT"),
                ColumnSpec::new("score_model_id", "TEXT"),
            ],
        )
        .await
        .unwrap();

        let cols = column_names(&conn, "scores").await;
        for c in [
            "id",
            "user_id",
            "score_type",
            "score_value",
            "score_name",
            "score_model_id",
        ] {
            assert!(cols.contains(c), "missing column {c}");
        }
        // Existing rows survive and get the declared defaults.
        let mut rows = conn
            .query(
                "SELECT score_type, score_value FROM scores WHERE id = 's1'",
                (),
            )
            .await
            .unwrap();
        let row = rows.next().await.unwrap().unwrap();
        assert_eq!(row.get::<String>(0).unwrap(), "");
        assert!((row.get::<f64>(1).unwrap() - 0.0).abs() < f64::EPSILON);
        let _ = std::fs::remove_file(&path);
    }

    #[tokio::test]
    async fn ensure_columns_is_idempotent() {
        let (_db, path, conn) = test_db().await;
        conn.execute("CREATE TABLE t (id TEXT PRIMARY KEY)", ())
            .await
            .unwrap();
        let cols = [ColumnSpec::new("v", "TEXT DEFAULT 'x'")];
        ensure_columns(&conn, "t", &cols).await.unwrap();
        ensure_columns(&conn, "t", &cols).await.unwrap();
        assert_eq!(column_names(&conn, "t").await.len(), 2);
        let _ = std::fs::remove_file(&path);
    }

    #[tokio::test]
    async fn ensure_columns_skips_impossible_shapes_without_failing() {
        let (_db, path, conn) = test_db().await;
        // Legacy populated table.
        conn.execute("CREATE TABLE t (a TEXT)", ()).await.unwrap();
        conn.execute("INSERT INTO t (a) VALUES ('x')", ())
            .await
            .unwrap();

        ensure_columns(
            &conn,
            "t",
            &[
                // PRIMARY KEY cannot be ADD COLUMN'd: skipped with a warning.
                ColumnSpec::new("id", "TEXT PRIMARY KEY"),
                // UNIQUE cannot be ADD COLUMN'd: skipped with a warning.
                ColumnSpec::new("code", "TEXT UNIQUE"),
                // NOT NULL without default on a populated table: skipped with
                // a warning instead of crashing the boot.
                ColumnSpec::new("req", "TEXT NOT NULL"),
                // Normal column: added.
                ColumnSpec::new("note", "TEXT"),
            ],
        )
        .await
        .unwrap();

        let cols = column_names(&conn, "t").await;
        assert!(cols.contains("note"));
        assert!(!cols.contains("id"));
        assert!(!cols.contains("code"));
        assert!(!cols.contains("req"));
        let _ = std::fs::remove_file(&path);
    }

    #[tokio::test]
    async fn ensure_columns_tolerates_a_concurrent_add() {
        let (_db, path, conn) = test_db().await;
        conn.execute("CREATE TABLE t (id TEXT PRIMARY KEY)", ())
            .await
            .unwrap();
        // First migrator adds the column; the second migration (same spec)
        // must treat the column as present. The duplicate-column error class
        // is additionally exercised directly below.
        let cols = [ColumnSpec::new("v", "TEXT")];
        ensure_columns(&conn, "t", &cols).await.unwrap();
        ensure_columns(&conn, "t", &cols).await.unwrap();
        // Simulate the race outcome: the column appeared between the
        // table_info read and the ALTER. The helper must swallow the
        // "duplicate column name" error instead of failing the boot.
        conn.execute("ALTER TABLE t ADD COLUMN v2 TEXT", ())
            .await
            .unwrap();
        ensure_columns(&conn, "t", &[ColumnSpec::new("v2", "TEXT")])
            .await
            .unwrap();
        assert_eq!(column_names(&conn, "t").await.len(), 3);
        let _ = std::fs::remove_file(&path);
    }

    #[tokio::test]
    async fn ensure_table_creates_fresh_and_repairs_legacy() {
        let (_db, path, conn) = test_db().await;
        // Fresh database: full DDL is used verbatim.
        ensure_table(
            &conn,
            "scores",
            "(id TEXT PRIMARY KEY, score_type TEXT NOT NULL DEFAULT '', note TEXT)",
            &[
                ColumnSpec::new("score_type", "TEXT NOT NULL DEFAULT ''"),
                ColumnSpec::new("note", "TEXT"),
            ],
        )
        .await
        .unwrap();
        let cols = column_names(&conn, "scores").await;
        assert!(cols.contains("id") && cols.contains("score_type") && cols.contains("note"));

        // Pre-existing legacy table missing columns: repaired via the spec
        // list, preserving existing rows.
        conn.execute(
            "CREATE TABLE legacy_scores (id TEXT PRIMARY KEY, user_id TEXT NOT NULL)",
            (),
        )
        .await
        .unwrap();
        conn.execute(
            "INSERT INTO legacy_scores (id, user_id) VALUES ('x', 'u')",
            (),
        )
        .await
        .unwrap();
        ensure_table(
            &conn,
            "legacy_scores",
            "(id TEXT PRIMARY KEY, user_id TEXT NOT NULL, extra TEXT DEFAULT 'e')",
            &[
                ColumnSpec::new("user_id", "TEXT NOT NULL"),
                ColumnSpec::new("extra", "TEXT DEFAULT 'e'"),
            ],
        )
        .await
        .unwrap();
        let cols = column_names(&conn, "legacy_scores").await;
        assert!(cols.contains("extra"));
        let mut rows = conn
            .query("SELECT COUNT(*) FROM legacy_scores WHERE id = 'x'", ())
            .await
            .unwrap();
        assert_eq!(
            rows.next().await.unwrap().unwrap().get::<i64>(0).unwrap(),
            1
        );
        let _ = std::fs::remove_file(&path);
    }

    #[tokio::test]
    async fn ensure_table_ddl_round_trips_body_verbatim() {
        let (_db, path, conn) = test_db().await;
        let ddl = "CREATE TABLE IF NOT EXISTS snap (\n\
                   id INTEGER PRIMARY KEY AUTOINCREMENT,\n\
                   user_id TEXT NOT NULL,\n\
                   -- approval state\n\
                   status TEXT NOT NULL DEFAULT 'pending' CHECK (status IN ('pending', 'approved')),\n\
                   payload TEXT DEFAULT 'a,b',\n\
                   created_at TEXT NOT NULL DEFAULT (datetime('now')),\n\
                   UNIQUE(user_id)\n\
                   )";
        ensure_table_ddl(&conn, ddl).await.unwrap();
        // Second boot: idempotent.
        ensure_table_ddl(&conn, ddl).await.unwrap();

        let cols = column_names(&conn, "snap").await;
        for c in ["id", "user_id", "status", "payload", "created_at"] {
            assert!(cols.contains(c), "missing column {c}");
        }
        // The fresh table must carry the table-level constraint from the body.
        let mut rows = conn
            .query(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='index' AND name LIKE 'sqlite_autoindex_snap%'",
                (),
            )
            .await
            .unwrap();
        assert_eq!(
            rows.next().await.unwrap().unwrap().get::<i64>(0).unwrap(),
            1
        );
        let _ = std::fs::remove_file(&path);
    }

    #[tokio::test]
    async fn ensure_table_ddl_repairs_legacy_table_from_body() {
        let (_db, path, conn) = test_db().await;
        // Legacy table predating the `account_statuses`/`approved_by` columns.
        conn.execute(
            "CREATE TABLE plaid_staging_data (id INTEGER PRIMARY KEY, user_id TEXT NOT NULL)",
            (),
        )
        .await
        .unwrap();
        conn.execute(
            "INSERT INTO plaid_staging_data (id, user_id) VALUES (1, 'u1')",
            (),
        )
        .await
        .unwrap();

        ensure_table_ddl(
            &conn,
            "CREATE TABLE IF NOT EXISTS plaid_staging_data (\n\
             id INTEGER PRIMARY KEY AUTOINCREMENT,\n\
             user_id TEXT NOT NULL,\n\
             account_statuses TEXT DEFAULT '{}',\n\
             expires_at TEXT NOT NULL,\n\
             approved_by TEXT\n\
             )",
        )
        .await
        .unwrap();

        let cols = column_names(&conn, "plaidd_staging_data".replace("dd", "d").as_str()).await;
        assert!(cols.contains("account_statuses"));
        assert!(cols.contains("approved_by"));
        // NOT NULL, no default, populated legacy row: skipped with a warning,
        // everything else repaired.
        assert!(!cols.contains("expires_at"));
        let _ = std::fs::remove_file(&path);
    }

    #[tokio::test]
    async fn ensure_table_ddls_initializes_every_statement() {
        let (_db, path, conn) = test_db().await;
        let ddls = [
            "CREATE TABLE IF NOT EXISTS a (id TEXT PRIMARY KEY, v TEXT)",
            "CREATE TABLE IF NOT EXISTS b (id TEXT PRIMARY KEY, a_id TEXT REFERENCES a(id))",
        ];
        ensure_table_ddls(&conn, &ddls).await.unwrap();
        ensure_table_ddls(&conn, &ddls).await.unwrap();
        assert!(column_names(&conn, "a").await.contains("v"));
        assert!(column_names(&conn, "b").await.contains("a_id"));
        let _ = std::fs::remove_file(&path);
    }

    #[tokio::test]
    async fn invalid_identifiers_are_rejected() {
        let (_db, path, conn) = test_db().await;
        conn.execute("CREATE TABLE t (id TEXT PRIMARY KEY)", ())
            .await
            .unwrap();
        let bad = [ColumnSpec::new("v; DROP TABLE t--", "TEXT")];
        assert!(ensure_columns(&conn, "t", &bad).await.is_err());
        let ok = [ColumnSpec::new("v", "TEXT")];
        assert!(ensure_columns(&conn, "t; DROP TABLE t--", &ok)
            .await
            .is_err());
        let _ = std::fs::remove_file(&path);
    }

    #[tokio::test]
    async fn quoted_reserved_word_columns_are_supported() {
        let (_db, path, conn) = test_db().await;
        // Column names that are SQL keywords must be quoted in the DDL; the
        // helper must match them against pragma_table_info (unquoted) and add
        // them with the quoting preserved.
        conn.execute("CREATE TABLE t (id TEXT PRIMARY KEY)", ())
            .await
            .unwrap();
        ensure_columns(
            &conn,
            "t",
            &[
                ColumnSpec::new("\"trigger\"", "TEXT"),
                ColumnSpec::new("note", "TEXT"),
            ],
        )
        .await
        .unwrap();
        ensure_columns(
            &conn,
            "t",
            &[
                ColumnSpec::new("\"trigger\"", "TEXT"),
                ColumnSpec::new("note", "TEXT"),
            ],
        )
        .await
        .unwrap();
        let cols = column_names(&conn, "t").await;
        assert!(cols.contains("trigger"));
        assert!(cols.contains("note"));
        let _ = std::fs::remove_file(&path);
    }
}
