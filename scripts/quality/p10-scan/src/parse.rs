//! Small string/path helpers shared by the metric visitors and the JSON
//! report renderer.

use syn::{Expr, Macro};

use crate::census::{LoopExitWalker, WhileFlag};
use syn::spanned::Spanned;
use syn::visit::Visit;

/// Escape a string for inclusion inside a JSON double-quoted literal.
pub(crate) fn jesc(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out
}

pub(crate) fn macro_name(mac: &Macro) -> String {
    mac.path.segments.last().map(|s| s.ident.to_string()).unwrap_or_default()
}

/// True only for genuine self-recursion: a bare `name(...)` or `Self::name(...)`
/// call. `OtherType::new(...)` inside `fn new` is an associated constructor,
/// not recursion.
pub(crate) fn is_self_call(expr: &Expr, name: &str) -> bool {
    match expr {
        Expr::Path(p) => {
            let segs = &p.path.segments;
            match segs.len() {
                1 => segs[0].ident == name,
                2 if segs[0].ident == "Self" => segs[1].ident == name,
                _ => false,
            }
        }
        _ => false,
    }
}

pub(crate) fn path_ends_with(expr: &Expr, name: &str) -> bool {
    match expr {
        Expr::Path(p) => p
            .path
            .segments
            .last()
            .map(|s| s.ident == name)
            .unwrap_or(false),
        _ => false,
    }
}

// --------------------------------------------------------- while classifier --

/// Ident/literal probe over a `while` condition. Classifies the loop as
/// bounded/exit-capable using the same categories the 09-08 manual triage
/// (`.freshcredit/quality-audit/while_loop_census.py`) validated: service
/// recv/select loops, pagination, retry bounds, drain patterns, state flags,
/// numeric compares, iterator state, io-read loops. Anything unmatched is
/// flagged for review; the gate keeps the flagged set from growing.
struct CondProbe {
    idents: Vec<String>,
    literal_true: bool,
    has_comparison: bool,
    has_numeric: bool,
}

impl<'ast> Visit<'ast> for CondProbe {
    fn visit_ident(&mut self, i: &'ast syn::Ident) {
        self.idents.push(i.to_string());
    }
    fn visit_lit_bool(&mut self, n: &'ast syn::LitBool) {
        if n.value {
            self.literal_true = true;
        }
    }
    fn visit_lit_int(&mut self, _: &'ast syn::LitInt) {
        self.has_numeric = true;
    }
    fn visit_expr_binary(&mut self, n: &'ast syn::ExprBinary) {
        use syn::BinOp::*;
        if matches!(n.op, Lt(_) | Le(_) | Gt(_) | Ge(_) | Ne(_) | Eq(_)) {
            self.has_comparison = true;
        }
        syn::visit::visit_expr_binary(self, n);
    }
}

/// Idents whose presence in a `while` condition marks the loop as belonging
/// to one of the triaged-bounded categories (see while_loop_census.py).
const BOUNDED_WHILE_IDENTS: &[&str] = &[
    // recv/select service loops
    "recv", "select", "next", "changed",
    // pagination / cursor
    "has_more", "next_page", "next_cursor", "page_token", "offset", "cursor", "page",
    // retry / attempt bounded
    "attempt", "retry", "tries", "retries", "backoff", "elapsed", "timeout", "deadline",
    "remaining", "tries_left",
    // drain (while let Some / pop / next)
    "pop", "pop_front", "next_entry", "join_next", "next_field",
    // state-flag (shutdown/cancel)
    "running", "shutdown", "cancelled", "canceled", "stop", "done", "finished", "alive",
    "closed", "disconnected",
    // compare progress (bounded)
    "len", "capacity", "size", "count", "is_empty",
    // iterator condition
    "peek", "has_next", "valid", "ready", "pending",
    // io/read loop
    "read", "fill_buf", "lines", "read_line",
];

/// Classify one `while` loop. Returns Some(WhileFlag) when no exit/bound
/// pattern is recognized (the loop joins the review population).
pub(crate) fn classify_while(cond: &syn::Expr, body: &syn::Block) -> Option<WhileFlag> {
    let line = cond.span().start().line;
    let mut probe = CondProbe {
        idents: Vec::new(),
        literal_true: false,
        has_comparison: false,
        has_numeric: false,
    };
    probe.visit_expr(cond);
    probe.idents.sort();
    probe.idents.dedup();
    let idents = probe.idents.join(" ");

    let mut exit = LoopExitWalker { found: false };
    exit.visit_block(body);
    let body_has_exit = exit.found;

    if body_has_exit {
        return None; // exit-capable regardless of condition shape
    }
    if probe
        .idents
        .iter()
        .any(|i| BOUNDED_WHILE_IDENTS.contains(&i.as_str()))
    {
        return None; // recognized bounded category
    }
    if probe.has_comparison && probe.has_numeric {
        return None; // numeric compare progress
    }
    if probe.literal_true {
        return Some(WhileFlag {
            line,
            idents,
            reason: "literal-true condition with no break/return in body".into(),
        });
    }
    Some(WhileFlag {
        line,
        idents,
        reason: "no recognized exit/bound pattern".into(),
    })
}
