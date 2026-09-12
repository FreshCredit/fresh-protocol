//! AST census: syn visitors that collect the per-function and per-file
//! structural metrics behind the Power-of-10 report.

use std::fs;
use std::path::{Path, PathBuf};

use syn::spanned::Spanned;
use syn::visit::Visit;
use syn::{Attribute, ExprLoop, ImplItemFn, Item, ItemFn, Macro, Pat, Visibility};

use crate::parse::{is_self_call, macro_name, path_ends_with};

// ------------------------------------------------------------ fn metrics --

#[derive(Default)]
pub(crate) struct FnMetrics {
    pub(crate) name: String,
    pub(crate) start_line: usize,
    pub(crate) end_line: usize,
    pub(crate) lines: usize,
    pub(crate) test_code: bool, // inside #[cfg(test)] context (mod, fn attr, or tests/ dir)
    pub(crate) asserts: usize,       // assert!, assert_eq!, assert_ne!, ...
    pub(crate) debug_asserts: usize, // debug_assert! family
    pub(crate) ensures: usize,       // anyhow::ensure! / ensure_or! style soft assertions
    pub(crate) tries: usize,         // `?` operator — checked-return handling
    pub(crate) loops_loop: usize,
    pub(crate) loops_while: usize,
    pub(crate) loops_for: usize,
    pub(crate) loop_without_exit: usize, // `loop {}` bodies with no break/return anywhere
    pub(crate) allocations: usize,
    pub(crate) clones: usize,
    pub(crate) unsafe_blocks: usize,
    pub(crate) recursive: bool,
    pub(crate) while_flags: Vec<WhileFlag>,
}

/// One `while` loop the classifier could not prove bounded/exit-capable.
#[derive(Default)]
pub(crate) struct WhileFlag {
    pub(crate) line: usize,
    pub(crate) idents: String, // sorted, de-duplicated ident signature of the condition
    pub(crate) reason: String,
}

pub(crate) struct FnVisitor {
    name: String,
    m: FnMetrics,
}

/// Counts break/return anywhere inside a loop body (approximation: includes
/// nested loops and closures; flagged as rough in the report).
pub(crate) struct LoopExitWalker {
    pub(crate) found: bool,
}

impl<'ast> Visit<'ast> for LoopExitWalker {
    fn visit_expr_break(&mut self, _: &'ast syn::ExprBreak) {
        self.found = true;
    }
    fn visit_expr_return(&mut self, _: &'ast syn::ExprReturn) {
        self.found = true;
    }
}

const ALLOC_FNS: &[&str] = &[
    "new", "with_capacity", "from", "from_iter", "boxed", "arc", "clone",
];
const ALLOC_MACROS: &[&str] = &["vec", "format", "format_args", "box", "str"];
const CLONE_METHODS: &[&str] = &["clone", "to_string", "to_owned", "to_vec", "into_owned", "copied", "cloned", "collect"];

impl<'ast> Visit<'ast> for FnVisitor {
    // Covers macros in BOTH statement position (`assert!(x);` is Stmt::Macro,
    // not Expr::Macro) and expression position.
    fn visit_macro(&mut self, node: &'ast Macro) {
        let n = macro_name(node);
        match n.as_str() {
            "assert" | "assert_eq" | "assert_ne" | "assert_matches" | "assert_err" | "assert_ok" => {
                self.m.asserts += 1
            }
            "debug_assert" | "debug_assert_eq" | "debug_assert_ne" => self.m.debug_asserts += 1,
            "ensure" | "ensure_or" | "bail" => self.m.ensures += 1,
            _ if ALLOC_MACROS.contains(&n.as_str()) => self.m.allocations += 1,
            _ => {}
        }
        syn::visit::visit_macro(self, node);
    }

    fn visit_expr_call(&mut self, node: &'ast syn::ExprCall) {
        if ALLOC_FNS.iter().any(|n| path_ends_with(&node.func, n)) {
            self.m.allocations += 1;
        }
        if is_self_call(&node.func, &self.name) {
            self.m.recursive = true;
        }
        syn::visit::visit_expr_call(self, node);
    }

    fn visit_expr_method_call(&mut self, node: &'ast syn::ExprMethodCall) {
        let m = node.method.to_string();
        if CLONE_METHODS.contains(&m.as_str()) {
            self.m.clones += 1;
        }
        syn::visit::visit_expr_method_call(self, node);
    }

    fn visit_expr_loop(&mut self, node: &'ast ExprLoop) {
        self.m.loops_loop += 1;
        let mut w = LoopExitWalker { found: false };
        w.visit_block(&node.body);
        if !w.found {
            self.m.loop_without_exit += 1;
        }
        syn::visit::visit_expr_loop(self, node);
    }

    fn visit_expr_while(&mut self, node: &'ast syn::ExprWhile) {
        self.m.loops_while += 1;
        if let Some(flag) = crate::parse::classify_while(&node.cond, &node.body) {
            self.m.while_flags.push(flag);
        }
        syn::visit::visit_expr_while(self, node);
    }

    fn visit_expr_for_loop(&mut self, node: &'ast syn::ExprForLoop) {
        self.m.loops_for += 1;
        syn::visit::visit_expr_for_loop(self, node);
    }

    fn visit_expr_unsafe(&mut self, node: &'ast syn::ExprUnsafe) {
        self.m.unsafe_blocks += 1;
        syn::visit::visit_expr_unsafe(self, node);
    }

    fn visit_expr_try(&mut self, node: &'ast syn::ExprTry) {
        self.m.tries += 1;
        syn::visit::visit_expr_try(self, node);
    }
}

#[derive(Default)]
pub(crate) struct FileMetrics {
    pub(crate) path: String,
    pub(crate) cfg_attrs: usize,
    pub(crate) macro_rules: usize,
    pub(crate) let_underscore: usize,
    pub(crate) dot_ok: usize,
    pub(crate) pub_items: usize,
    pub(crate) restricted_items: usize,
    pub(crate) private_items: usize,
    pub(crate) raw_ptr_types: usize,
    pub(crate) bare_fn_types: usize,
    pub(crate) unsafe_item_fns: usize,
    pub(crate) functions: Vec<FnMetrics>,
}

#[derive(Default)]
pub(crate) struct FileVisitor {
    pub(crate) m: FileMetrics,
    /// Depth of `#[cfg(test)]` contexts (mods and fns) we're currently inside.
    pub(crate) test_ctx: usize,
}

/// True when an attribute list contains `#[cfg(test)]`/`#[cfg_attr(test, …)]`
/// or a compound cfg mentioning `test`.
fn is_cfg_test(attrs: &[Attribute]) -> bool {
    attrs.iter().any(|a| {
        (a.path().is_ident("cfg") || a.path().is_ident("cfg_attr"))
            && matches!(&a.meta, syn::Meta::List(l) if l.tokens.to_string().contains("test"))
    })
}

/// Test files by convention: `…/tests/*.rs` or `*_test.rs`.
fn is_test_path(path: &str) -> bool {
    path.split('/').any(|c| c == "tests") || path.ends_with("_test.rs")
}

fn count_cfg(attrs: &[Attribute]) -> usize {
    attrs.iter().filter(|a| a.path().is_ident("cfg") || a.path().is_ident("cfg_attr")).count()
}

fn vis_bucket(v: &Visibility) -> u8 {
    match v {
        Visibility::Public(_) => 0,
        Visibility::Restricted(_) => 1,
        _ => 2,
    }
}

/// Walks types appearing anywhere in the file (signatures, struct fields).
struct TypeWalker {
    raw_ptrs: usize,
    bare_fns: usize,
}

impl<'ast> Visit<'ast> for TypeWalker {
    fn visit_type_ptr(&mut self, _: &'ast syn::TypePtr) {
        self.raw_ptrs += 1;
    }
    fn visit_type_bare_fn(&mut self, _: &'ast syn::TypeBareFn) {
        self.bare_fns += 1;
    }
}

impl<'ast> Visit<'ast> for FileVisitor {
    fn visit_item_fn(&mut self, node: &'ast ItemFn) {
        self.record_fn(&node.sig, &node.block);
        let is_test = self.test_ctx > 0 || is_cfg_test(&node.attrs) || is_test_path(&self.m.path);
        if let Some(f) = self.m.functions.last_mut() {
            f.test_code = is_test;
        }
        if is_test {
            self.test_ctx += 1;
        }
        syn::visit::visit_item_fn(self, node);
        if is_test {
            self.test_ctx -= 1;
        }
    }

    fn visit_impl_item_fn(&mut self, node: &'ast ImplItemFn) {
        self.record_fn(&node.sig, &node.block);
        let is_test = self.test_ctx > 0 || is_cfg_test(&node.attrs);
        if let Some(f) = self.m.functions.last_mut() {
            f.test_code = is_test;
        }
        if is_test {
            self.test_ctx += 1;
        }
        syn::visit::visit_impl_item_fn(self, node);
        if is_test {
            self.test_ctx -= 1;
        }
    }

    fn visit_local(&mut self, node: &'ast syn::Local) {
        if let Pat::Wild(_) = &node.pat {
            self.m.let_underscore += 1;
        }
        syn::visit::visit_local(self, node);
    }

    fn visit_expr_method_call(&mut self, node: &'ast syn::ExprMethodCall) {
        if node.method == "ok" {
            self.m.dot_ok += 1;
        }
        syn::visit::visit_expr_method_call(self, node);
    }

    fn visit_item(&mut self, node: &'ast Item) {
        self.m.cfg_attrs += count_cfg(match node {
            Item::Fn(i) => &i.attrs,
            Item::Struct(i) => &i.attrs,
            Item::Enum(i) => &i.attrs,
            Item::Mod(i) => &i.attrs,
            Item::Trait(i) => &i.attrs,
            Item::Impl(i) => &i.attrs,
            Item::Const(i) => &i.attrs,
            Item::Static(i) => &i.attrs,
            Item::Type(i) => &i.attrs,
            Item::Use(i) => &i.attrs,
            _ => &[],
        });
        if let Item::Macro(i) = node {
            if i.mac.path.is_ident("macro_rules") {
                self.m.macro_rules += 1;
            }
        }
        // Rule 6: declaration at smallest reasonable scope.
        if let Some(v) = match node {
            Item::Fn(i) => Some(&i.vis),
            Item::Struct(i) => Some(&i.vis),
            Item::Enum(i) => Some(&i.vis),
            Item::Mod(i) => Some(&i.vis),
            Item::Trait(i) => Some(&i.vis),
            Item::Const(i) => Some(&i.vis),
            Item::Static(i) => Some(&i.vis),
            Item::Type(i) => Some(&i.vis),
            _ => None,
        } {
            match vis_bucket(v) {
                0 => self.m.pub_items += 1,
                1 => self.m.restricted_items += 1,
                _ => self.m.private_items += 1,
            }
        }
        let pushed = match node {
            Item::Mod(i) if is_cfg_test(&i.attrs) => {
                self.test_ctx += 1;
                true
            }
            _ => false,
        };
        syn::visit::visit_item(self, node);
        if pushed {
            self.test_ctx -= 1;
        }
    }
}

impl FileVisitor {
    fn record_fn(&mut self, sig: &syn::Signature, block: &syn::Block) {
        let name = sig.ident.to_string();
        let start = sig.span().start().line;
        let end = block.span().end().line;
        let mut v = FnVisitor {
            name: name.clone(),
            m: FnMetrics {
                name,
                start_line: start,
                end_line: end,
                lines: end.saturating_sub(start) + 1,
                ..Default::default()
            },
        };
        if sig.unsafety.is_some() {
            self.m.unsafe_item_fns += 1;
        }
        v.visit_block(block);
        // Signature-level types (Rule 9 scan).
        let mut tw = TypeWalker {
            raw_ptrs: 0,
            bare_fns: 0,
        };
        for arg in &sig.inputs {
            if let syn::FnArg::Typed(syn::PatType { ty, .. }) = arg {
                tw.visit_type(ty);
            }
        }
        tw.visit_return_type(&sig.output);
        self.m.raw_ptr_types += tw.raw_ptrs;
        self.m.bare_fn_types += tw.bare_fns;
        self.m.functions.push(v.m);
    }
}

// ---------------------------------------------------------------- walker --

fn is_skipped_dir(p: &Path) -> bool {
    p.file_name()
        .map(|n| {
            let n = n.to_string_lossy();
            n == "target" || n == "node_modules" || n == ".git" || n == ".cargo" || n == "dist" || n == "build"
        })
        .unwrap_or(false)
}

pub(crate) fn collect_rs_files(root: &Path, out: &mut Vec<PathBuf>) {
    if is_skipped_dir(root) {
        return;
    }
    // Use symlink_metadata semantics: never follow symlinks. Several repo
    // dirs (e.g. crates/testnet-agent-bridge/crates -> ../../crates) are
    // symlinks that would recurse infinitely and duplicate every file.
    let Ok(entries) = fs::read_dir(root) else { return };
    for e in entries.flatten() {
        let Ok(ft) = e.file_type() else { continue };
        if ft.is_symlink() {
            continue;
        }
        let p = e.path();
        if ft.is_dir() {
            collect_rs_files(&p, out);
        } else if p.extension().map(|e| e == "rs").unwrap_or(false) {
            out.push(p);
        }
    }
}
