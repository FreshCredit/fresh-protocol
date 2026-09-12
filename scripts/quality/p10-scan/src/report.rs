//! JSON rendering for the p10 census report and the stderr text summary.
//!
//! The JSON schema emitted here is consumed by the quality gate and audit
//! reports; keep field names and ordering stable.

use crate::census::{FileMetrics, FnMetrics};
use crate::parse::jesc;

pub(crate) struct Totals {
    pub(crate) files: usize,
    pub(crate) parse_errors: Vec<String>,
    pub(crate) functions: usize,
    pub(crate) lines_over_60: usize,
    pub(crate) lines_over_200: usize,
    pub(crate) max_fn_lines: usize,
    pub(crate) max_fn_name: String,
    pub(crate) recursive_fns: usize,
    pub(crate) bare_loops: usize,
    pub(crate) bare_loops_without_exit: usize,
    pub(crate) while_loops: usize,
    pub(crate) while_flagged: usize,
    pub(crate) for_loops: usize,
    pub(crate) fns_with_zero_asserts: usize,
    pub(crate) prod_fns_zero_asserts: usize,
    pub(crate) total_asserts: usize,
    pub(crate) total_alloc_sites: usize,
    pub(crate) total_clones: usize,
    pub(crate) unsafe_fns: usize,
    pub(crate) unsafe_blocks: usize,
}

impl Default for Totals {
    fn default() -> Self {
        Totals {
            files: 0,
            parse_errors: Vec::new(),
            functions: 0,
            lines_over_60: 0,
            lines_over_200: 0,
            max_fn_lines: 0,
            max_fn_name: String::new(),
            recursive_fns: 0,
            bare_loops: 0,
            bare_loops_without_exit: 0,
            while_loops: 0,
            while_flagged: 0,
            for_loops: 0,
            fns_with_zero_asserts: 0,
            prod_fns_zero_asserts: 0,
            total_asserts: 0,
            total_alloc_sites: 0,
            total_clones: 0,
            unsafe_fns: 0,
            unsafe_blocks: 0,
        }
    }
}

impl Totals {
    /// Fold one file's per-function metrics into the running totals.
    pub(crate) fn accumulate(&mut self, m: &FileMetrics) {
        self.functions += m.functions.len();
        for fun in &m.functions {
            if fun.lines > 60 {
                self.lines_over_60 += 1;
            }
            if fun.lines > 200 {
                self.lines_over_200 += 1;
            }
            if fun.lines > self.max_fn_lines {
                self.max_fn_lines = fun.lines;
                self.max_fn_name = format!("{}:{} ({})", m.path, fun.start_line, fun.name);
            }
            if fun.recursive {
                self.recursive_fns += 1;
            }
            self.bare_loops += fun.loops_loop;
            self.bare_loops_without_exit += fun.loop_without_exit;
            self.while_loops += fun.loops_while;
            self.while_flagged += fun.while_flags.len();
            self.for_loops += fun.loops_for;
            if fun.asserts + fun.ensures == 0 {
                self.fns_with_zero_asserts += 1;
                if !fun.test_code {
                    self.prod_fns_zero_asserts += 1;
                }
            }
            self.total_asserts += fun.asserts;
            self.total_alloc_sites += fun.allocations;
            self.total_clones += fun.clones;
            self.unsafe_blocks += fun.unsafe_blocks;
        }
        self.unsafe_fns += m.unsafe_item_fns;
    }
}

pub(crate) fn json_array(items: &[String]) -> String {
    let inner: Vec<String> = items.iter().map(|s| format!("\"{}\"", jesc(s))).collect();
    format!("[{}]", inner.join(", "))
}

fn totals_json(t: &Totals) -> String {
    format!(
        "{{\n    \"files\": {},\n    \"functions\": {},\n    \"fns_over_60_lines\": {},\n    \"fns_over_200_lines\": {},\n    \"max_fn_lines\": {},\n    \"max_fn\": \"{}\",\n    \"recursive_fns\": {},\n    \"loop_constructs\": {},\n    \"loops_without_break_or_return\": {},\n    \"while_loops\": {},\n    \"while_flagged\": {},\n    \"for_loops\": {},\n    \"fns_with_zero_assert_or_ensure\": {},\n    \"prod_fns_with_zero_assert_or_ensure\": {},\n    \"assert_sites\": {},\n    \"allocation_like_sites\": {},\n    \"clone_like_calls\": {},\n    \"unsafe_fns\": {},\n    \"unsafe_blocks\": {}\n  }}",
        t.files,
        t.functions,
        t.lines_over_60,
        t.lines_over_200,
        t.max_fn_lines,
        jesc(&t.max_fn_name),
        t.recursive_fns,
        t.bare_loops,
        t.bare_loops_without_exit,
        t.while_loops,
        t.while_flagged,
        t.for_loops,
        t.fns_with_zero_asserts,
        t.prod_fns_zero_asserts,
        t.total_asserts,
        t.total_alloc_sites,
        t.total_clones,
        t.unsafe_fns,
        t.unsafe_blocks
    )
}

fn while_flags_json(f: &FnMetrics) -> String {
    if f.while_flags.is_empty() {
        return "[]".into();
    }
    let items: Vec<String> = f
        .while_flags
        .iter()
        .map(|w| {
            format!(
                "{{\"line\": {}, \"idents\": \"{}\", \"reason\": \"{}\"}}",
                w.line,
                jesc(&w.idents),
                jesc(&w.reason)
            )
        })
        .collect();
    format!("[{}]", items.join(", "))
}

fn fn_json(f: &FnMetrics) -> String {
    format!(
        "{{\"name\": \"{}\", \"start\": {}, \"end\": {}, \"lines\": {}, \"test\": {}, \"asserts\": {}, \"debug_asserts\": {}, \"ensures\": {}, \"tries\": {}, \"loop\": {}, \"loop_no_exit\": {}, \"while\": {}, \"while_flagged\": {}, \"for\": {}, \"allocs\": {}, \"clones\": {}, \"unsafe_blocks\": {}, \"recursive\": {}}}",
        jesc(&f.name),
        f.start_line,
        f.end_line,
        f.lines,
        f.test_code,
        f.asserts,
        f.debug_asserts,
        f.ensures,
        f.tries,
        f.loops_loop,
        f.loop_without_exit,
        f.loops_while,
        while_flags_json(f),
        f.loops_for,
        f.allocations,
        f.clones,
        f.unsafe_blocks,
        f.recursive
    )
}

pub(crate) fn file_json(m: &FileMetrics) -> String {
    let fns: Vec<String> = m.functions.iter().map(fn_json).collect();
    format!(
        "    {{\n      \"path\": \"{}\",\n      \"cfg_attrs\": {},\n      \"macro_rules\": {},\n      \"let_underscore\": {},\n      \"dot_ok\": {},\n      \"pub_items\": {},\n      \"restricted_items\": {},\n      \"private_items\": {},\n      \"raw_ptr_types\": {},\n      \"bare_fn_types\": {},\n      \"unsafe_item_fns\": {},\n      \"functions\": [{}]\n    }}",
        jesc(&m.path),
        m.cfg_attrs,
        m.macro_rules,
        m.let_underscore,
        m.dot_ok,
        m.pub_items,
        m.restricted_items,
        m.private_items,
        m.raw_ptr_types,
        m.bare_fn_types,
        m.unsafe_item_fns,
        fns.join(", ")
    )
}

/// Assemble the top-level JSON document.
pub(crate) fn render_doc(args: &[String], totals: &Totals, json_files: &str) -> String {
    format!(
        "{{\n  \"tool\": \"p10-scan 0.1.0\",\n  \"roots\": {},\n  \"totals\": {},\n  \"parse_errors\": {},\n  \"files\": [\n{}\n  ]\n}}",
        json_array(args),
        totals_json(totals),
        json_array(&totals.parse_errors),
        json_files
    )
}

/// Human-readable census summary on stderr.
pub(crate) fn print_summary(totals: &Totals) {
    eprintln!("=== P10 census summary ===");
    eprintln!("files parsed:              {}", totals.files);
    eprintln!("parse errors:              {}", totals.parse_errors.len());
    eprintln!("functions:                 {}", totals.functions);
    eprintln!("fns > 60 lines:            {}", totals.lines_over_60);
    eprintln!("fns > 200 lines:           {}", totals.lines_over_200);
    eprintln!(
        "largest fn:                {} ({} lines)",
        totals.max_fn_name, totals.max_fn_lines
    );
    eprintln!("recursive fns:             {}", totals.recursive_fns);
    eprintln!("`loop` constructs:         {}", totals.bare_loops);
    eprintln!("  without break/return:    {}", totals.bare_loops_without_exit);
    eprintln!("`while` loops:             {}", totals.while_loops);
    eprintln!("  flagged (unproven bound):{}", totals.while_flagged);
    eprintln!("`for` loops:               {}", totals.for_loops);
    eprintln!("fns w/ zero assert/ensure: {}", totals.fns_with_zero_asserts);
    eprintln!("  of which production code:{}", totals.prod_fns_zero_asserts);
    eprintln!("assert! sites:             {}", totals.total_asserts);
    eprintln!("alloc-ish sites:           {}", totals.total_alloc_sites);
    eprintln!("clone-ish calls:           {}", totals.total_clones);
    eprintln!("unsafe fns:                {}", totals.unsafe_fns);
    eprintln!("unsafe blocks:             {}", totals.unsafe_blocks);
}
