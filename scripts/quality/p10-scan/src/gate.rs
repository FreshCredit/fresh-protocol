//! `--gate` mode: ratchet enforcement over the P10 census.
//!
//! Compares the fresh census against a committed baseline
//! (`gate-baseline.json` next to this crate, overridable with
//! `--baseline <path>`). The gate fails when:
//!
//!   1. `fns_over_200_lines` exceeds the baseline (hard cap, ratchets down
//!      only when a refactor PR intentionally lowers it),
//!   2. `unsafe_blocks` exceeds the baseline (test-only unsafe drift check),
//!   3. parsed `files` drops below the baseline (parse regression detector),
//!   4. production (non-`#[cfg(test)]`) functions with zero assert/ensure
//!      *increase* — no-decrease ratchet on Rule-5 coverage,
//!   5. any self-call (recursion-candidate) fn not in the triaged whitelist,
//!   6. any `loop` without break/return not in the service-loop whitelist,
//!   7. any `while` loop the classifier cannot prove bounded/exit-capable
//!      whose (path, condition-ident) signature is not in the triaged list.
//!
//! Whitelists encode the 2026-09-08 manual triage (report
//! `.freshcredit/quality-audit/04-POWER-OF-10.md`): new entries require
//! human review and an explicit baseline update in the same PR.

use std::fs;

use crate::census::FileMetrics;
use crate::report::Totals;

pub(crate) struct Baseline {
    max_fns_over_200: usize,
    max_unsafe_blocks: usize,
    min_files: usize,
    prod_zero_asserts: usize,
    recursion_whitelist: Vec<String>,     // "path:name"
    loop_no_exit_whitelist: Vec<String>,  // "path:name"
    while_flagged_whitelist: Vec<String>, // "path|idents"
}

// ---------------------------------------------------- minimal JSON scraping --
// The baseline file is written by scripts/quality/p10-scan/update-baseline.py
// from scanner output, so a tolerant key-based scraper is sufficient and
// avoids a serde dependency in this detached crate.

fn scrape_number(src: &str, key: &str) -> Option<usize> {
    let pat = format!("\"{}\":", key);
    let start = src.find(&pat)? + pat.len();
    let rest = src[start..].trim_start();
    let end = rest
        .find(|c: char| !c.is_ascii_digit())
        .unwrap_or(rest.len());
    rest[..end].parse().ok()
}

fn scrape_string_array(src: &str, key: &str) -> Vec<String> {
    let pat = format!("\"{}\":", key);
    let Some(mut i) = src.find(&pat) else {
        return Vec::new();
    };
    i += pat.len();
    let rest = &src[i..];
    let Some(open) = rest.find('[') else {
        return Vec::new();
    };
    let Some(close) = rest[open..].find(']') else {
        return Vec::new();
    };
    rest[open + 1..open + close]
        .split(',')
        .map(|s| s.trim().trim_matches('"').to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

pub(crate) fn load_baseline(path: &str) -> Result<Baseline, String> {
    let src = fs::read_to_string(path).map_err(|e| format!("cannot read baseline {path}: {e}"))?;
    Ok(Baseline {
        max_fns_over_200: scrape_number(&src, "max_fns_over_200")
            .ok_or_else(|| format!("baseline {path}: missing max_fns_over_200"))?,
        max_unsafe_blocks: scrape_number(&src, "max_unsafe_blocks")
            .ok_or_else(|| format!("baseline {path}: missing max_unsafe_blocks"))?,
        min_files: scrape_number(&src, "min_files")
            .ok_or_else(|| format!("baseline {path}: missing min_files"))?,
        prod_zero_asserts: scrape_number(&src, "prod_zero_asserts")
            .ok_or_else(|| format!("baseline {path}: missing prod_zero_asserts"))?,
        recursion_whitelist: scrape_string_array(&src, "recursion_whitelist"),
        loop_no_exit_whitelist: scrape_string_array(&src, "loop_no_exit_whitelist"),
        while_flagged_whitelist: scrape_string_array(&src, "while_flagged_whitelist"),
    })
}

// ------------------------------------------------------------------- checks --

fn push_unique(out: &mut Vec<String>, s: String) {
    if !out.contains(&s) {
        out.push(s);
    }
}

/// Run all gate checks. Returns the list of failures (empty = pass).
pub(crate) fn run_gate(t: &Totals, files: &[FileMetrics], b: &Baseline) -> Vec<String> {
    let mut failures = Vec::new();

    // 1–3: absolute thresholds
    if t.lines_over_200 > b.max_fns_over_200 {
        failures.push(format!(
            "fns_over_200_lines = {} exceeds baseline cap {} (split the new fns or raise the cap deliberately)",
            t.lines_over_200, b.max_fns_over_200
        ));
    }
    if t.unsafe_blocks > b.max_unsafe_blocks {
        failures.push(format!(
            "unsafe_blocks = {} exceeds baseline {} (unsafe must stay test-only)",
            t.unsafe_blocks, b.max_unsafe_blocks
        ));
    }
    if t.files < b.min_files {
        failures.push(format!(
            "parsed files = {} below baseline {} (parse regressions?)",
            t.files, b.min_files
        ));
    }

    // 4: Rule-5 ratchet — production zero-assert fns must not increase
    if t.prod_fns_zero_asserts > b.prod_zero_asserts {
        failures.push(format!(
            "prod fns with zero assert/ensure = {} increased vs baseline {} (add asserts or ensures)",
            t.prod_fns_zero_asserts, b.prod_zero_asserts
        ));
    }

    // 5–7: whitelist membership
    let mut new_recursion = Vec::new();
    let mut new_no_exit = Vec::new();
    let mut new_while = Vec::new();
    for f in files {
        for fun in &f.functions {
            let key = format!("{}:{}", f.path, fun.name);
            if fun.recursive && !b.recursion_whitelist.contains(&key) {
                push_unique(&mut new_recursion, key.clone());
            }
            if fun.loop_without_exit > 0 && !b.loop_no_exit_whitelist.contains(&key) {
                push_unique(&mut new_no_exit, key);
            }
            for w in &fun.while_flags {
                let key = format!("{}|{}", f.path, w.idents);
                if !b.while_flagged_whitelist.contains(&key) {
                    push_unique(&mut new_while, format!("{} ({}): {}", key, w.line, w.reason));
                }
            }
        }
    }
    if !new_recursion.is_empty() {
        failures.push(format!(
            "{} self-call (recursion-candidate) fn(s) not in triaged whitelist — triage per 04-POWER-OF-10.md R1 and add to baseline:\n    {}",
            new_recursion.len(),
            new_recursion.join("\n    ")
        ));
    }
    if !new_no_exit.is_empty() {
        failures.push(format!(
            "{} `loop` without break/return fn(s) not in service-loop whitelist:\n    {}",
            new_no_exit.len(),
            new_no_exit.join("\n    ")
        ));
    }
    if !new_while.is_empty() {
        failures.push(format!(
            "{} `while` loop(s) with no proven bound/exit not in triaged list — review per while_loop_census.py categories:\n    {}",
            new_while.len(),
            new_while.join("\n    ")
        ));
    }

    failures
}

pub(crate) fn print_gate_report(t: &Totals, b: &Baseline, failures: &[String]) {
    println!("=== P10 gate report ===");
    let check = |ok: bool| if ok { "PASS" } else { "FAIL" };
    println!(
        "[threshold] fns_over_200_lines  {}/{}  {}",
        t.lines_over_200,
        b.max_fns_over_200,
        check(t.lines_over_200 <= b.max_fns_over_200)
    );
    println!(
        "[threshold] unsafe_blocks       {}/{}  {}",
        t.unsafe_blocks,
        b.max_unsafe_blocks,
        check(t.unsafe_blocks <= b.max_unsafe_blocks)
    );
    println!(
        "[threshold] parsed files        {}/{}  {}",
        t.files,
        b.min_files,
        check(t.files >= b.min_files)
    );
    println!(
        "[ratchet]   prod zero-assert    {}/{}  {}",
        t.prod_fns_zero_asserts,
        b.prod_zero_asserts,
        check(t.prod_fns_zero_asserts <= b.prod_zero_asserts)
    );
    println!(
        "[whitelist] recursion           {} known / {} allowed",
        t.recursive_fns,
        b.recursion_whitelist.len()
    );
    println!(
        "[whitelist] loop-no-exit        {} known / {} allowed",
        t.bare_loops_without_exit, b.loop_no_exit_whitelist.len()
    );
    println!(
        "[whitelist] while-flagged       {} known / {} allowed",
        t.while_flagged,
        b.while_flagged_whitelist.len()
    );
    if failures.is_empty() {
        println!("\nP10 GATE: PASS");
    } else {
        println!("\nP10 GATE: FAIL ({} check(s))", failures.len());
    }
}
