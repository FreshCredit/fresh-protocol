//! NASA "Power of 10" safety-critical code pattern scanner.
//!
//! Walks Rust source trees and, for every function, collects the structural
//! metrics needed to evaluate the 10 rules (Holzmann 2006). This is a
//! brute-force pattern census: it reports raw counts so humans (or later
//! fixer scripts) can triage. It deliberately performs no fixes.
//!
//! Usage: p10-scan [--gate [--baseline PATH]] [ROOT...] [OUT_JSON]
//!   Defaults roots: crates apps packages xtask
//!   Output: JSON to OUT_JSON (or stdout) + text summary on stderr.
//!   --gate: non-zero exit when the census violates gate-baseline.json
//!     (thresholds + triage whitelists). See gate.rs for the check list.

use std::fs;
use std::path::{Path, PathBuf};

use syn::visit::Visit;

mod census;
mod gate;
mod parse;
mod report;

use census::{collect_rs_files, FileMetrics, FileVisitor};
use report::{file_json, print_summary, render_doc, Totals};

fn main() {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let mut out_path: Option<PathBuf> = None;
    let mut gate_mode = false;
    let mut baseline_path = "scripts/quality/p10-scan/gate-baseline.json".to_string();

    // flag extraction (flags may appear anywhere)
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--gate" => {
                gate_mode = true;
                args.remove(i);
            }
            "--baseline" => {
                if i + 1 >= args.len() {
                    eprintln!("--baseline requires a path argument");
                    std::process::exit(2);
                }
                baseline_path = args[i + 1].clone();
                args.remove(i);
                args.remove(i);
            }
            _ => i += 1,
        }
    }
    if let Some(last) = args.last() {
        if last.ends_with(".json") {
            out_path = args.pop().map(PathBuf::from);
        }
    }
    if args.is_empty() {
        args = vec![
            "crates".into(),
            "apps".into(),
            "packages".into(),
            "xtask".into(),
        ];
    }

    let mut files = Vec::new();
    for r in &args {
        collect_rs_files(Path::new(r), &mut files);
    }
    files.sort();

    let mut totals = Totals::default();
    let mut json_files = String::new();
    let mut kept: Vec<FileMetrics> = Vec::new();
    let t0 = std::time::Instant::now();

    for (i, f) in files.iter().enumerate() {
        if i % 50 == 0 {
            eprintln!("[{} files, {:.1}s] {}", i, t0.elapsed().as_secs_f64(), f.display());
        }
        let Ok(src) = fs::read_to_string(f) else { continue };
        let rel = f.to_string_lossy().to_string();
        let parsed = match syn::parse_file(&src) {
            Ok(p) => p,
            Err(e) => {
                totals.parse_errors.push(format!("{}: {}", rel, e));
                continue;
            }
        };
        totals.files += 1;

        let mut fv = FileVisitor {
            m: FileMetrics {
                path: rel,
                ..Default::default()
            },
            ..Default::default()
        };
        fv.visit_file(&parsed);
        totals.accumulate(&fv.m);

        json_files.push_str(&file_json(&fv.m));
        json_files.push(',');
        if gate_mode {
            kept.push(fv.m);
        }
    }
    json_files.pop(); // trailing comma

    let doc = render_doc(&args, &totals, &json_files);

    match out_path {
        Some(p) => {
            if let Some(parent) = p.parent() {
                let _ = fs::create_dir_all(parent);
            }
            fs::write(&p, &doc).expect("write json");
            eprintln!("wrote {}", p.display());
        }
        None => println!("{}", doc),
    }

    print_summary(&totals);

    if gate_mode {
        match gate::load_baseline(&baseline_path) {
            Ok(baseline) => {
                let failures = gate::run_gate(&totals, &kept, &baseline);
                gate::print_gate_report(&totals, &baseline, &failures);
                if !failures.is_empty() {
                    for f in &failures {
                        eprintln!("GATE FAILURE: {}", f);
                    }
                    std::process::exit(1);
                }
            }
            Err(e) => {
                eprintln!("P10 GATE: cannot evaluate — {}", e);
                std::process::exit(2);
            }
        }
    }
}
