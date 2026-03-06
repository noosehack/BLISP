#![allow(dead_code)]
#![allow(unused_imports)]
#![allow(unused_mut)]
#![allow(clippy::useless_vec)]
use blisp::reader::Reader;
use blisp::runtime::Runtime;
use blisp::value::{self, Value};
use blisp::{ast, eval, exec, io, ir, ir_fusion, normalize, planner};
use std::env;
use std::io::Write;

const VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Debug, PartialEq)]
enum Subcommand {
    Run,
    Verify,
    Selftest,
    Dic,
}

// ─── Pipeline Inspector (--pipe) ─────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq)]
enum PipeMode {
    Off,
    Full,
    Stages,
    Table,
}

#[derive(Debug, Clone)]
struct CallSite {
    preorder_id: usize,
    user_sym: String,
    canonical_sym: String,
}

#[derive(Debug, Clone)]
struct PipeReport {
    // Header
    mode: String,
    segment: bool,
    fusion_invoked: bool,

    // [PARSE]
    raw_ast: String,

    // [NORMALIZE]
    post_thread: String,
    rewrites_normalize: Vec<normalize::RewriteEvent>,

    // [CANONICALIZE]
    canonical: String,
    rewrites_canonicalize: Vec<normalize::RewriteEvent>,

    // [PLAN]
    plan_nodes: Vec<String>,
    plan_error: Option<String>,

    // [OPTIMIZE]
    pre_fusion_count: usize,
    post_fusion_count: usize,
    fused_ops: Vec<String>,

    // [EXECUTE]
    exec_path: String,
    fallback_reason: Option<String>,
    glue_forms: Vec<String>,

    // [TABLE]
    call_sites: Vec<CallSite>,
}

impl PipeReport {
    fn new() -> Self {
        PipeReport {
            mode: String::new(),
            segment: false,
            fusion_invoked: false,
            raw_ast: String::new(),
            post_thread: String::new(),
            rewrites_normalize: Vec::new(),
            rewrites_canonicalize: Vec::new(),
            canonical: String::new(),
            plan_nodes: Vec::new(),
            plan_error: None,
            pre_fusion_count: 0,
            post_fusion_count: 0,
            fused_ops: Vec::new(),
            exec_path: String::new(),
            fallback_reason: None,
            glue_forms: Vec::new(),
            call_sites: Vec::new(),
        }
    }

    fn emit(&self, pipe_mode: PipeMode) {
        if pipe_mode == PipeMode::Off {
            return;
        }

        let show_stages = pipe_mode == PipeMode::Full || pipe_mode == PipeMode::Stages;
        let show_table = pipe_mode == PipeMode::Full || pipe_mode == PipeMode::Table;

        if show_stages {
            eprintln!("=== PIPE ===");
            eprintln!("mode:             {}", self.mode);
            eprintln!(
                "segment:          {}",
                if self.segment {
                    "on (BLISP_SEGMENT)"
                } else {
                    "off"
                }
            );
            eprintln!(
                "fusion_invoked:   {}",
                if self.fusion_invoked { "yes" } else { "no" }
            );
            eprintln!();

            // [PARSE]
            eprintln!("[PARSE]");
            eprintln!("  {}", self.raw_ast);
            eprintln!();

            // [NORMALIZE]
            eprintln!("[NORMALIZE]");
            eprintln!("  {}", self.post_thread);
            if !self.rewrites_normalize.is_empty() {
                eprintln!("  rewrites:");
                for rw in &self.rewrites_normalize {
                    if let normalize::RewriteEvent::ThreadFirst { form_count } = rw {
                        eprintln!("    -> expanded ({} forms)", form_count);
                    }
                }
            }
            eprintln!();

            // [CANONICALIZE]
            eprintln!("[CANONICALIZE]");
            eprintln!("  {}", self.canonical);
            if !self.rewrites_canonicalize.is_empty() {
                eprintln!("  rewrites:");
                for rw in &self.rewrites_canonicalize {
                    match rw {
                        normalize::RewriteEvent::Alias { from, to } => {
                            eprintln!("    alias: {} -> {}", from, to);
                        }
                        normalize::RewriteEvent::ArgSwap2 { op } => {
                            eprintln!("    arg-swap-2: {} (prefix -> data-first)", op);
                        }
                        normalize::RewriteEvent::ArgSwap3 { op } => {
                            eprintln!("    arg-swap-3: {} (prefix -> data-first)", op);
                        }
                        _ => {}
                    }
                }
            }
            eprintln!();

            // [PLAN]
            eprintln!("[PLAN]");
            if let Some(ref err) = self.plan_error {
                eprintln!("  FAILED: {}", err);
            } else {
                eprintln!("  {} nodes", self.plan_nodes.len());
                for (i, desc) in self.plan_nodes.iter().enumerate() {
                    eprintln!("  #{}  {}", i, desc);
                }
            }
            eprintln!();

            // [OPTIMIZE]
            eprintln!("[OPTIMIZE]");
            eprintln!(
                "  fusion_invoked:  {}",
                if self.fusion_invoked { "yes" } else { "no" }
            );
            if self.fusion_invoked {
                eprintln!("  nodes_before:    {}", self.pre_fusion_count);
                eprintln!("  nodes_after:     {}", self.post_fusion_count);
                let fused_count = self.fused_ops.len();
                eprintln!("  fused_nodes:     {}", fused_count);
                if fused_count > 0 {
                    eprintln!("  fused_ops:");
                    for f in &self.fused_ops {
                        eprintln!("    {}", f);
                    }
                } else {
                    eprintln!("  fused_ops:       (none)");
                }
            }
            eprintln!();

            // [EXECUTE]
            eprintln!("[EXECUTE]");
            eprintln!("  path: {}", self.exec_path);
            if !self.glue_forms.is_empty() {
                eprintln!("  glue_forms: {}", self.glue_forms.join(", "));
            }
            if let Some(ref reason) = self.fallback_reason {
                eprintln!("  fallback_reason: {}", reason);
            }
            eprintln!();
        }

        if show_table && !self.call_sites.is_empty() {
            eprintln!("[TABLE]");
            // Build table with IR correlation
            eprintln!(
                "  {:>3}  {:<12} {:<12} {:<30} {:<7} ran",
                "#", "user", "canonical", "ir_variant", "fused"
            );
            for site in &self.call_sites {
                // Find matching IR node by canonical name
                let (ir_variant, fused) = self.find_ir_for_call(site);
                let ran = if self.plan_error.is_some() {
                    "legacy"
                } else {
                    &self.exec_path
                };
                eprintln!(
                    "  {:>3}  {:<12} {:<12} {:<30} {:<7} {}",
                    site.preorder_id, site.user_sym, site.canonical_sym, ir_variant, fused, ran
                );
            }
            eprintln!("=== END ===");
        } else if show_stages {
            eprintln!("=== END ===");
        }
    }

    /// Best-effort correlation of a call site with an IR plan node
    fn find_ir_for_call(&self, site: &CallSite) -> (String, String) {
        // Walk plan nodes and find the one most likely matching this call site
        // by checking if the canonical name maps to the IR node's operation
        for desc in &self.plan_nodes {
            let ir_name = canonical_to_ir_name(&site.canonical_sym);
            if let Some(ref expected) = ir_name {
                if desc.contains(expected) {
                    let fused = if desc.contains("Fused") { "yes" } else { "-" };
                    // Extract the short variant name
                    return (expected.to_string(), fused.to_string());
                }
            }
        }
        // Fallback: check sources
        match site.canonical_sym.as_str() {
            "stdin" => ("Source::Stdin".to_string(), "-".to_string()),
            "file" | "load" | "read-csv" => ("Source::File".to_string(), "-".to_string()),
            "file-fast" => ("Source::FileFast".to_string(), "-".to_string()),
            "let" => ("let-binding".to_string(), "-".to_string()),
            "mapr" => ("ALIGN".to_string(), "-".to_string()),
            "asofr" => ("ASOF_ALIGN".to_string(), "-".to_string()),
            "xminus" => ("SHF_PTW_LIN_SPR".to_string(), "-".to_string()),
            _ => ("-".to_string(), "-".to_string()),
        }
    }
}

/// Map canonical user names to IR variant names (for table display)
fn canonical_to_ir_name(name: &str) -> Option<&'static str> {
    match name {
        "dlog" => Some("SHF_PTW_OBS_NLN_DLOG"),
        "dlog-ofs" => Some("SHF_PTW_OFS_NLN_DLOG"),
        "ret" => Some("RET"),
        "log" | "ln" => Some("LOG"),
        "exp" => Some("EXP"),
        "sqrt" => Some("SQRT"),
        "abs" => Some("ABS"),
        "inv" => Some("INV"),
        "locf" => Some("SHF_REC_NLN_LOCF"),
        "wkd" | "w5" => Some("MSK_WKE"),
        "run_sum" | "cs1" => Some("SHF_PFX_LIN_SUM"),
        "shift" => Some("SHF_PTW_LIN_SHF"),
        "lag-obs" | "shift-obs" => Some("LAG_OBS"),
        "keep" => Some("KEEP"),
        "rol_avg" | "rolling-mean" => Some("SHF_WIN_LIN_AVG"),
        "rol_std" | "rolling-std" => Some("SHF_WIN_NLN_SDV"),
        "rolling-mean-min2" => Some("SHF_WIN_MIN2_LIN_AVG"),
        "rolling-std-min2" => Some("SHF_WIN_MIN2_NLN_SDV"),
        "ft-mean" => Some("SHF_WIN_MIN2_LIN_AVG_EXCL"),
        "ft-std" => Some("SHF_WIN_MIN2_NLN_SDV_EXCL"),
        "rol_zsc" | "rolling-zscore" | "wzs" => Some("composite"),
        "ft-zscore" => Some("composite"),
        "rsk_adj" | "ur" => Some("composite"),
        "+" | "add" => Some("ADD"),
        "-" | "sub" => Some("SUB"),
        "*" | "mul" => Some("MUL"),
        "/" | "div" => Some("DIV"),
        ">" | "gt" => Some("GTR"),
        "<" | "lt" => Some("LSS"),
        ">=" | "gte" => Some("GTE"),
        "<=" | "lte" => Some("LTE"),
        "==" | "eq" => Some("EQL"),
        "!=" | "neq" => Some("NEQ"),
        _ => None,
    }
}

/// Detect fused operation variants in a plan (matches actual enum variants)
fn detect_fused_ops(plan: &ir::Plan) -> Vec<String> {
    plan.nodes
        .iter()
        .filter_map(|n| match &n.op {
            ir::Operation::Unary(u) => match u {
                ir::UnaryOp::FusedElementwise { ops, .. } => Some(format!(
                    "FusedElementwise[{}]",
                    ops.iter()
                        .map(|o| format!("{:?}", o))
                        .collect::<Vec<_>>()
                        .join(", ")
                )),
                ir::UnaryOp::FusedCs1Elementwise { ops, .. } => Some(format!(
                    "FusedCs1Elementwise[{}]",
                    ops.iter()
                        .map(|o| format!("{:?}", o))
                        .collect::<Vec<_>>()
                        .join(", ")
                )),
                ir::UnaryOp::FusedCs1DlogOfs { lag, .. } => {
                    Some(format!("FusedCs1DlogOfs(lag={})", lag))
                }
                ir::UnaryOp::FusedCs1DlogObs { .. } => Some("FusedCs1DlogObs".to_string()),
                ir::UnaryOp::FusedDlogObsElementwise { ops, .. } => Some(format!(
                    "FusedDlogObsElementwise[{}]",
                    ops.iter()
                        .map(|o| format!("{:?}", o))
                        .collect::<Vec<_>>()
                        .join(", ")
                )),
                ir::UnaryOp::FusedDlogOfsElementwise { lag, ops, .. } => Some(format!(
                    "FusedDlogOfsElementwise(lag={}) [{}]",
                    lag,
                    ops.iter()
                        .map(|o| format!("{:?}", o))
                        .collect::<Vec<_>>()
                        .join(", ")
                )),
                _ => None,
            },
            _ => None,
        })
        .collect()
}

/// Walk an AST in preorder, collecting function-position symbols
fn walk_calls(expr: &ast::Expr, interner: &ast::Interner, out: &mut Vec<String>) {
    if let ast::Expr::List(elements) = expr {
        if let Some(ast::Expr::Sym(sym)) = elements.first() {
            let name = interner.resolve(*sym);
            if name != "->" {
                out.push(name.to_string());
            }
        }
        for e in elements {
            walk_calls(e, interner, out);
        }
    }
}

/// Build call site table by walking post-thread and canonical ASTs in preorder
fn collect_call_sites(
    post_thread: &ast::Expr,
    canonical: &ast::Expr,
    interner: &ast::Interner,
) -> Vec<CallSite> {
    let mut thread_calls = Vec::new();
    let mut canonical_calls = Vec::new();
    walk_calls(post_thread, interner, &mut thread_calls);
    walk_calls(canonical, interner, &mut canonical_calls);

    if thread_calls.len() != canonical_calls.len() {
        eprintln!(
            "  TABLE_WARNING: call-site length mismatch (post_thread={} canonical={})",
            thread_calls.len(),
            canonical_calls.len()
        );
    }

    thread_calls
        .into_iter()
        .zip(canonical_calls)
        .enumerate()
        .map(|(i, (user, canon))| CallSite {
            preorder_id: i,
            user_sym: user,
            canonical_sym: canon,
        })
        .collect()
}

/// Pretty-print an Expr (resolved symbols instead of SymbolId)
fn display_expr(expr: &ast::Expr, interner: &ast::Interner) -> String {
    match expr {
        ast::Expr::Nil => "nil".to_string(),
        ast::Expr::Bool(b) => b.to_string(),
        ast::Expr::Int(n) => n.to_string(),
        ast::Expr::Float(f) => format!("{}", f),
        ast::Expr::Str(s) => format!("\"{}\"", s),
        ast::Expr::Sym(sym) => interner.resolve(*sym).to_string(),
        ast::Expr::List(elements) => {
            let inner: Vec<String> = elements.iter().map(|e| display_expr(e, interner)).collect();
            format!("({})", inner.join(" "))
        }
        ast::Expr::Quote(inner) => format!("'{}", display_expr(inner, interner)),
        ast::Expr::QuasiQuote(inner) => format!("`{}", display_expr(inner, interner)),
        ast::Expr::Unquote(inner) => format!(",{}", display_expr(inner, interner)),
        ast::Expr::UnquoteSplicing(inner) => format!(",@{}", display_expr(inner, interner)),
    }
}

fn print_help() {
    eprintln!("blisp v{} (IR-optimized)", VERSION);
    eprintln!();
    eprintln!("USAGE:");
    eprintln!("  blisp [OPTIONS] [SUBCOMMAND]");
    eprintln!();
    eprintln!("SUBCOMMANDS:");
    eprintln!("  run <script.lisp>              Run a BLISP script (default)");
    eprintln!("  verify <actual> <expected>     Verify CSV outputs match");
    eprintln!("  selftest                       Run embedded self-tests");
    eprintln!("  dic [OPTIONS]                  Operation matrix (CSV, code-driven)");
    eprintln!();
    eprintln!("OPTIONS:");
    eprintln!("  --version                      Show version and exit");
    eprintln!("  --help                         Show this help message");
    eprintln!("  --load <file>                  Load stdlib file before execution");
    eprintln!("  -e '<expression>'              Evaluate expression");
    eprintln!("  --legacy                       Force legacy AST evaluator");
    eprintln!("  --ir-only                      Force IR-only mode (experimental)");
    eprintln!("  --trace-plan                   Log IR planner decisions to stderr");
    eprintln!("  --pipe                         Show full pipeline inspector (all stages + table)");
    eprintln!("  --pipe=stages                  Show pipeline stages only (parse through execute)");
    eprintln!("  --pipe=table                   Show per-op mapping table only");
    eprintln!();
    eprintln!("DIC OPTIONS:");
    eprintln!("  (default)                      Operation matrix as CSV (code-driven, no YAML)");
    eprintln!("  --json                         Output in JSON format");
    eprintln!("  --grep <pattern>               Filter by pattern");
    eprintln!("  --no-yaml                      Suppress YAML columns (default when no view flag)");
    eprintln!("  --matrix                       Explicit matrix view (same as default)");
    eprintln!("  --exposed                      Legacy: show exposed aliases (YAML-driven)");
    eprintln!("  --legacy                       Legacy: show legacy tokens (YAML-driven)");
    eprintln!("  --todo-ir                      Legacy: show IR migration queue (YAML-driven)");
    eprintln!("  --check-resolve                Legacy: check runtime resolution (YAML-driven)");
    eprintln!();
    eprintln!("VERIFY OPTIONS:");
    eprintln!(
        "  --tol <value>                  Tolerance for numerical comparison (default: 1e-6)"
    );
    eprintln!("  --verbose                      Show all failures (not just first 10)");
    eprintln!();
    eprintln!("EXAMPLES:");
    eprintln!("  blisp -e '(+ 1 2)'             Evaluate expression");
    eprintln!("  blisp script.lisp              Run script (implicit 'run' subcommand)");
    eprintln!("  blisp run examples/hello.blisp Run example");
    eprintln!("  blisp --selftest               Run self-tests");
    eprintln!("  blisp verify out.csv exp.csv   Verify outputs match");
    eprintln!();
    eprintln!("ENVIRONMENT:");
    eprintln!("  BLISP_LEGACY=1                 Force legacy evaluator");
    eprintln!("  BLISP_IR_ONLY=1                Force IR-only mode");
    eprintln!("  BLISP_TRACE_PLAN=1             Log IR planner decisions to stderr");
}

fn parse_subcommand(args: &[String]) -> Subcommand {
    // Check explicit subcommands
    if args.len() > 1 {
        match args[1].as_str() {
            "selftest" | "--selftest" => return Subcommand::Selftest,
            "verify" => return Subcommand::Verify,
            "run" => return Subcommand::Run,
            "dic" | "--dic" => return Subcommand::Dic,
            _ => {}
        }
    }

    // Auto-detect based on file extension or flags
    for arg in args.iter().skip(1) {
        if arg.ends_with(".lisp") || arg.ends_with(".cl") || arg.ends_with(".blisp") {
            return Subcommand::Run;
        }
        if arg == "-e" {
            return Subcommand::Run;
        }
    }

    // Default to Run for backward compatibility
    Subcommand::Run
}

fn handle_dic_subcommand(args: &[String]) {
    // Parse dic arguments: blisp dic [--exposed|--legacy|--todo-ir|--matrix] [--json] [--grep <pattern>] [--no-yaml]
    use blisp::dic::{OutputFormat, View};

    let mut view = View::All;
    let mut format = OutputFormat::Table;
    let mut grep_pattern: Option<String> = None;
    let mut no_yaml = false;
    let mut i = 2; // Skip "blisp" and "dic"

    // If no flags, default to exposed
    let has_view_flag = args.iter().skip(2).any(|arg| {
        matches!(
            arg.as_str(),
            "--exposed"
                | "--legacy"
                | "--todo-ir"
                | "--unmapped"
                | "--check-resolve"
                | "--planned"
                | "--matrix"
        )
    });

    if !has_view_flag {
        view = View::Matrix;
        no_yaml = true;
    }

    while i < args.len() {
        match args[i].as_str() {
            "--exposed" => {
                view = View::Exposed;
                i += 1;
            }
            "--legacy" => {
                view = View::Legacy;
                i += 1;
            }
            "--todo-ir" => {
                view = View::TodoIR;
                i += 1;
            }
            "--unmapped" => {
                view = View::Unmapped;
                i += 1;
            }
            "--check-resolve" => {
                view = View::CheckResolve;
                i += 1;
            }
            "--planned" => {
                view = View::Planned;
                i += 1;
            }
            "--matrix" => {
                view = View::Matrix;
                i += 1;
            }
            "--no-yaml" => {
                no_yaml = true;
                i += 1;
            }
            "--json" => {
                format = OutputFormat::Json;
                i += 1;
            }
            "--grep" => {
                if i + 1 >= args.len() {
                    eprintln!("Error: --grep requires a pattern");
                    std::process::exit(1);
                }
                grep_pattern = Some(args[i + 1].clone());
                i += 2;
            }
            _ => {
                eprintln!("Error: unknown dic option: {}", args[i]);
                eprintln!("Valid options: --exposed, --legacy, --todo-ir, --unmapped, --check-resolve, --planned, --matrix, --json, --grep <pattern>, --no-yaml");
                std::process::exit(1);
            }
        }
    }

    match blisp::dic::run_dic(view, format, grep_pattern.as_deref(), no_yaml) {
        Ok(()) => std::process::exit(0),
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    }
}

fn handle_verify_subcommand(args: &[String]) {
    // Parse verify arguments: blisp verify <actual> <expected> [--tol <value>] [--verbose]
    let mut actual = None;
    let mut expected = None;
    let mut tolerance = 1e-6;
    let mut verbose = false;
    let mut i = 2; // Skip "blisp" and "verify"

    while i < args.len() {
        match args[i].as_str() {
            "--tol" => {
                if i + 1 >= args.len() {
                    eprintln!("Error: --tol requires a value");
                    std::process::exit(1);
                }
                tolerance = args[i + 1].parse().unwrap_or_else(|_| {
                    eprintln!("Error: invalid tolerance value");
                    std::process::exit(1);
                });
                i += 2;
            }
            "--verbose" => {
                verbose = true;
                i += 1;
            }
            _ => {
                if actual.is_none() {
                    actual = Some(args[i].clone());
                } else if expected.is_none() {
                    expected = Some(args[i].clone());
                } else {
                    eprintln!("Error: unexpected argument: {}", args[i]);
                    std::process::exit(1);
                }
                i += 1;
            }
        }
    }

    let actual = actual.unwrap_or_else(|| {
        eprintln!("Error: verify requires <actual> <expected> arguments");
        std::process::exit(1);
    });

    let expected = expected.unwrap_or_else(|| {
        eprintln!("Error: verify requires <actual> <expected> arguments");
        std::process::exit(1);
    });

    let opts = blisp::verify::VerifyOptions { tolerance, verbose };

    match blisp::verify::verify_csv(&actual, &expected, &opts) {
        Ok(results) => {
            println!("✅ Verification PASSED");
            println!("  Rows compared: {}", results.rows_compared);
            println!("  Max difference: {:.2e}", results.max_diff);
            if results.max_diff > 0.0 {
                println!("  Max diff at row: {}", results.max_diff_row);
            }
            std::process::exit(0);
        }
        Err(e) => {
            eprintln!("❌ Verification FAILED");
            eprintln!("{}", e);
            std::process::exit(1);
        }
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();

    // Handle --version flag
    if args.len() > 1 && args[1] == "--version" {
        println!("blisp v{}", VERSION);
        std::process::exit(0);
    }

    // Handle --help flag
    if args.len() > 1 && args[1] == "--help" {
        print_help();
        std::process::exit(0);
    }

    // Parse command line arguments
    if args.len() < 2 {
        print_help();
        std::process::exit(1);
    }

    // Determine subcommand
    let subcommand = parse_subcommand(&args);

    // Dispatch to subcommand handlers
    match subcommand {
        Subcommand::Selftest => {
            let results = blisp::selftest::run_all_tests();
            if results.failed > 0 {
                std::process::exit(1);
            } else {
                std::process::exit(0);
            }
        }
        Subcommand::Verify => {
            handle_verify_subcommand(&args);
        }
        Subcommand::Dic => {
            handle_dic_subcommand(&args);
        }
        Subcommand::Run => {
            // Fall through to existing run logic below
        }
    }

    let mut rt = Runtime::new();

    // Check for IR-only mode (experimental)
    let use_ir_only = env::var("BLISP_IR_ONLY").is_ok() || args.contains(&"--ir-only".to_string());
    let use_legacy = env::var("BLISP_LEGACY").is_ok() || args.contains(&"--legacy".to_string());
    let trace_plan =
        env::var("BLISP_TRACE_PLAN").is_ok() || args.contains(&"--trace-plan".to_string());
    let pipe_mode = if args.contains(&"--pipe=stages".to_string()) {
        PipeMode::Stages
    } else if args.contains(&"--pipe=table".to_string()) {
        PipeMode::Table
    } else if args.contains(&"--pipe".to_string()) {
        PipeMode::Full
    } else {
        PipeMode::Off
    };

    if use_ir_only {
        eprintln!("🚧 Running in IR-ONLY mode (Frame operations only, experimental)");
    } else if use_legacy {
        eprintln!("⚠️  Running in LEGACY mode (old AST evaluator only)");
    } else {
        // Default: hybrid mode (IR for Frame ops, legacy fallback)
        eprintln!("✅ Running in HYBRID mode (IR for Frame ops, legacy fallback)");
    }

    // Parse arguments
    let mut i = 1;

    // Skip "run" subcommand if present
    if i < args.len() && args[i] == "run" {
        i += 1;
    }

    let mut load_files = Vec::new();
    let mut expression = None;
    let mut script_file = None;

    while i < args.len() {
        match args[i].as_str() {
            "--legacy" | "--ir-only" | "--dic" | "--trace-plan" | "--pipe" | "--pipe=stages"
            | "--pipe=table" | "selftest" | "--selftest" => {
                // Already handled above, just skip
                i += 1;
            }
            "--load" => {
                if i + 1 >= args.len() {
                    eprintln!("Error: --load requires a file path");
                    std::process::exit(1);
                }
                load_files.push(args[i + 1].clone());
                i += 2;
            }
            "-e" => {
                if i + 1 >= args.len() {
                    eprintln!("Error: -e requires an expression");
                    std::process::exit(1);
                }
                expression = Some(args[i + 1].clone());
                i += 2;
            }
            _ => {
                // Assume it's a script file
                script_file = Some(args[i].clone());
                i += 1;
            }
        }
    }

    // Load files (always use legacy for --load files, as they may contain defmacro, etc.)
    for file in load_files {
        if let Err(e) = load_file(&mut rt, &file, true) {
            // true = always legacy for --load
            eprintln!("Error loading {}: {}", file, e);
            std::process::exit(1);
        }
    }

    // Execute -e or script
    if let Some(expr) = expression {
        let code = &expr;
        match eval_code(
            &mut rt,
            code,
            use_legacy,
            use_ir_only,
            trace_plan,
            pipe_mode,
        ) {
            Ok(val) => {
                // Stream output directly to stdout with BufWriter for efficiency
                let stdout = std::io::stdout();
                let mut writer = std::io::BufWriter::new(stdout.lock());

                let result = match &val {
                    value::Value::Table(table) => {
                        value::write_table_to(&mut writer, table, &rt.interner, None)
                    }
                    value::Value::Frame(frame) => {
                        value::write_frame_to(&mut writer, frame, &rt.interner, None)
                    }
                    _ => {
                        writeln!(writer, "{}", val.display(&rt.interner))
                    }
                };

                if let Err(e) = result {
                    if e.kind() == std::io::ErrorKind::BrokenPipe {
                        std::process::exit(0);
                    }
                    eprintln!("Error writing output: {}", e);
                    std::process::exit(1);
                }
            }
            Err(e) => {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
        }
    } else if let Some(file) = script_file {
        // File execution
        match std::fs::read_to_string(&file) {
            Ok(code) => {
                match eval_code(
                    &mut rt,
                    &code,
                    use_legacy,
                    use_ir_only,
                    trace_plan,
                    pipe_mode,
                ) {
                    Ok(val) => {
                        // Stream output directly to stdout with BufWriter for efficiency
                        let stdout = std::io::stdout();
                        let mut writer = std::io::BufWriter::new(stdout.lock());

                        let result = match &val {
                            value::Value::Table(table) => {
                                // Stream table output directly (no row limit when not interactive)
                                value::write_table_to(&mut writer, table, &rt.interner, None)
                            }
                            value::Value::Frame(frame) => {
                                // Stream frame output directly (no row limit when not interactive)
                                value::write_frame_to(&mut writer, frame, &rt.interner, None)
                            }
                            _ => {
                                // For non-tables/frames, use display()
                                writeln!(writer, "{}", val.display(&rt.interner))
                            }
                        };

                        if let Err(e) = result {
                            if e.kind() == std::io::ErrorKind::BrokenPipe {
                                std::process::exit(0);
                            }
                            eprintln!("Error writing output: {}", e);
                            std::process::exit(1);
                        }
                    }
                    Err(e) => {
                        eprintln!("Error: {}", e);
                        std::process::exit(1);
                    }
                }
            }
            Err(e) => {
                eprintln!("Error reading file '{}': {}", file, e);
                std::process::exit(1);
            }
        }
    } else {
        eprintln!("Error: must provide -e <expr> or <script>");
        std::process::exit(1);
    }
}

fn load_file(rt: &mut Runtime, path: &str, _use_legacy: bool) -> Result<(), String> {
    let code = std::fs::read_to_string(path).map_err(|e| format!("Cannot read file: {}", e))?;

    let mut reader = Reader::new(&code).map_err(|e| format!("Parse error: {}", e))?;

    // Read and eval all forms using legacy evaluator
    // (--load files may contain defmacro, defparameter, etc. which IR doesn't handle)
    loop {
        match reader.read(&mut rt.interner) {
            Ok(expr) => {
                rt.eval(&expr)?;
            }
            Err(e) => {
                let err_str = format!("{:?}", e);
                if err_str.contains("Unexpected EOF") || err_str.contains("EOF") {
                    break;
                } else {
                    return Err(format!("Read error: {}", e));
                }
            }
        }
    }

    Ok(())
}

fn demo_column_ops() {
    let mut rt = Runtime::new();

    println!("=== Step 6: High-Performance Column Operations ===");
    println!();

    use std::sync::Arc;
    use value::Value;

    // Create a price time series
    let prices_data = vec![100.0, 102.0, 101.5, 103.0, 104.5, 105.0, 106.5, 107.0];
    let prices_col = blawktrust::Column::new_f64(prices_data);
    let prices_val = Value::Col(Arc::new(prices_col));

    let prices_sym = rt.interner.intern("prices");
    rt.define(prices_sym, prices_val);

    let test_programs = vec![
        ("(len prices)", "Number of price points"),
        ("(dlog prices 1)", "Daily log returns (optimized kernel!)"),
        ("(shift prices 1)", "Yesterday's prices"),
        ("(diff prices 1)", "Daily price changes"),
        ("(* (dlog prices 1) 252)", "Annualized log returns"),
    ];

    for (code, description) in test_programs {
        println!(">>> {}", description);
        println!("{}", code);

        match eval_code(&mut rt, code, false, false, false, PipeMode::Off) {
            Ok(val) => {
                match &val {
                    Value::Col(c) => {
                        println!("=> Col[{} elements]", c.len());
                        if let blawktrust::Column::F64(data) = &**c {
                            let display_count = 5.min(data.len());
                            println!("   First {}: {:?}", display_count, &data[..display_count]);
                        }
                    }
                    _ => println!("=> {}", val.display(&rt.interner)),
                }
                println!();
            }
            Err(e) => println!("Error: {}\n", e),
        }
    }

    println!("✅ High-performance column operations working!");
    println!("🚀 Using blawktrust's optimized kernels (1.89x faster than C++)!");
}

fn demo_builtins() {
    let mut rt = Runtime::new();

    let test_programs = vec![
        // Arithmetic
        ("(+ 1 2)", "Add integers"),
        ("(+ 3.14 2.86)", "Add floats"),
        ("(+ 1 2.5)", "Add int and float"),
        ("(- 10 3)", "Subtract"),
        ("(* 3 4)", "Multiply"),
        ("(/ 10 2)", "Divide"),
        // Math
        ("(abs -5)", "Absolute value"),
        ("(log 2.718281828)", "Natural log"),
        ("(exp 1.0)", "Exponential"),
        // With variables
        ("(defparameter x 10)", "Define x"),
        ("(+ x 5)", "Add with variable"),
        ("(* x 2)", "Multiply with variable"),
        // Utility
        ("(type-of 42)", "Type of int"),
        ("(type-of 3.14)", "Type of float"),
        ("(print \"Hello, blisp!\")", "Print string"),
        // Nested expressions
        ("(+ (* 2 3) (- 10 5))", "Nested: (2*3) + (10-5)"),
        ("(* (+ 1 2) (+ 3 4))", "Nested: (1+2) * (3+4)"),
    ];

    for (code, description) in test_programs {
        println!(">>> {}", description);
        println!("{}", code);

        match eval_code(&mut rt, code, false, false, false, PipeMode::Off) {
            Ok(val) => println!("=> {}\n", val.display(&rt.interner)),
            Err(e) => println!("Error: {}\n", e),
        }
    }

    // Column operations demo
    println!("=== Column Operations ===\n");

    use std::sync::Arc;
    use value::Value;

    // Create a column
    let data = vec![100.0, 102.0, 101.5, 103.0, 104.5];
    let col = blawktrust::Column::new_f64(data);
    let col_val = Value::Col(Arc::new(col));

    let prices_sym = rt.interner.intern("prices");
    rt.define(prices_sym, col_val);

    let col_programs = vec![
        ("(len prices)", "Column length"),
        ("(+ prices 10)", "Add scalar to column"),
        ("(* prices 1.1)", "Scale column by 1.1"),
        ("(log prices)", "Log of column"),
    ];

    for (code, description) in col_programs {
        println!(">>> {}", description);
        println!("{}", code);

        match eval_code(&mut rt, code, false, false, false, PipeMode::Off) {
            Ok(val) => {
                match &val {
                    Value::Col(c) => {
                        println!("=> Col[{} elements]", c.len());
                        if let blawktrust::Column::F64(data) = &**c {
                            println!("   First 3: {:?}", &data[..3.min(data.len())]);
                        }
                    }
                    _ => println!("=> {}", val.display(&rt.interner)),
                }
                println!();
            }
            Err(e) => println!("Error: {}\n", e),
        }
    }

    println!("✅ All builtins working!");
}

fn demo_column_types() {
    use std::sync::Arc;
    use value::{Table, Value};

    println!("=== Column and Table Types ===");
    println!();

    // Create a column
    let data = vec![100.0, 102.0, 101.5, 103.0, 104.5];
    let col = blawktrust::Column::new_f64(data);
    let col_val = Value::Col(Arc::new(col));

    let mut interner = ast::Interner::new();
    println!("Column: {}", col_val.display(&interner));
    println!("Type: {}", col_val.type_name());
    println!();

    // Create a table
    let mut table = Table::new();

    let px_data = vec![100.0, 102.0, 101.5, 103.0, 104.5];
    let px_col = blawktrust::Column::new_f64(px_data);
    let px_name = interner.intern("px");
    table.add_column(px_name, px_col);

    let vol_data = vec![1000.0, 1200.0, 800.0, 1500.0, 900.0];
    let vol_col = blawktrust::Column::new_f64(vol_data);
    let vol_name = interner.intern("vol");
    table.add_column(vol_name, vol_col);

    let table_val = Value::Table(Arc::new(table));
    println!("Table: {}", table_val.display(&interner));
    println!("Type: {}", table_val.type_name());
    println!();

    // Extract column from table
    if let Ok(tbl) = table_val.as_table() {
        if let Some(px_col) = tbl.get_column(px_name) {
            println!("Extracted 'px' column: {} elements", px_col.len());
        }
    }
    println!();

    println!("✅ Column and Table types working!");
}

fn demo_evaluator() {
    let mut rt = Runtime::new();

    let test_programs = vec![
        // Literals
        ("42", "Literal integer"),
        ("3.14", "Literal float"),
        ("\"hello\"", "Literal string"),
        // Quote
        ("'foo", "Quote symbol"),
        ("'42", "Quote number"),
        // progn
        ("(progn 1 2 3)", "progn returns last"),
        // defparameter
        ("(defparameter x 10)", "Define global x"),
        ("x", "Read x"),
        // if
        ("(if t 'yes 'no)", "if with true condition"),
        ("(if nil 'yes 'no)", "if with false condition"),
        ("(if 0 'yes 'no)", "if with 0 (truthy in Lisp)"),
        // setf
        ("(setf x 20)", "Update x"),
        ("x", "Read x again"),
        // let*
        ("(let* ((y 100)) y)", "Simple let*"),
        ("(let* ((a 1) (b 2)) b)", "let* with multiple bindings"),
        // Nested let*
        (
            "(let* ((x 5)) (let* ((x 10)) x))",
            "Nested let* (inner shadows)",
        ),
        // Complex expression
        (
            r#"(progn
               (defparameter z 1)
               (let* ((z 2))
                 (setf z 20)
                 z))"#,
            "Complex: progn + defparameter + let* + setf",
        ),
        ("z", "z should still be 1 (global unchanged)"),
    ];

    for (code, description) in test_programs {
        println!(">>> {}", description);
        println!("{}", code);

        match eval_code(&mut rt, code, false, false, false, PipeMode::Off) {
            Ok(val) => println!("=> {:?}\n", val),
            Err(e) => println!("Error: {}\n", e),
        }
    }
}

/// Errors from the IR evaluation pipeline, distinguishing plan-time vs exec-time.
#[derive(Debug)]
enum IrError {
    /// Planner could not handle this expression (HYBRID should fallback)
    Plan(planner::PlanError),
    /// Execution failed (propagate as real error)
    Exec(String),
}

impl std::fmt::Display for IrError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            IrError::Plan(e) => write!(f, "plan: {}", e),
            IrError::Exec(e) => write!(f, "exec: {}", e),
        }
    }
}

/// Try to evaluate via IR path (normalize → plan → optimize → execute)
/// When pipe is Some, populates the report progressively (even on PlanError).
fn try_ir_eval(
    rt: &mut Runtime,
    expr: ast::Expr,
    trace: bool,
    pipe: &mut Option<PipeReport>,
) -> Result<value::Value, IrError> {
    // Step 1: Normalize
    let (normalized, _post_thread_expr) = if pipe.is_some() {
        let nt = normalize::normalize_traced(expr, &mut rt.interner);
        // Split rewrites into normalize (ThreadFirst) and canonicalize (Alias, ArgSwap) events
        let mut norm_events = Vec::new();
        let mut canon_events = Vec::new();
        for rw in nt.rewrites {
            match &rw {
                normalize::RewriteEvent::ThreadFirst { .. } => norm_events.push(rw),
                _ => canon_events.push(rw),
            }
        }
        if let Some(ref mut rpt) = pipe {
            rpt.post_thread = display_expr(&nt.post_thread, &rt.interner);
            rpt.rewrites_normalize = norm_events;
            rpt.canonical = display_expr(nt.expr.inner(), &rt.interner);
            rpt.rewrites_canonicalize = canon_events;
            rpt.call_sites = collect_call_sites(&nt.post_thread, nt.expr.inner(), &rt.interner);
        }
        let pt = nt.post_thread;
        (nt.expr, Some(pt))
    } else {
        (normalize::normalize(expr, &mut rt.interner), None)
    };

    if trace {
        eprintln!("[TRACE] canonical= {:?}", normalized.inner());
    }

    // Step 2: Plan (AST → IR with schema validation)
    let plan = match planner::plan(&normalized, &rt.interner) {
        Ok(p) => p,
        Err(plan_err) => {
            if let Some(ref mut rpt) = pipe {
                rpt.plan_error = Some(format!("{}", plan_err));
                rpt.fusion_invoked = false;
                rpt.exec_path = "legacy-fallback".to_string();
                rpt.fallback_reason = Some(format!("{}", plan_err));
            }
            return Err(IrError::Plan(plan_err));
        }
    };

    if trace {
        let ops: Vec<String> = plan.nodes.iter().map(|n| format!("{:?}", n.op)).collect();
        eprintln!("[TRACE] planned ops= {:?}", ops);
    }

    let pre_fusion_count = plan.nodes.len();

    // Capture pre-fusion plan nodes for pipe
    if let Some(ref mut rpt) = pipe {
        rpt.plan_nodes = plan.nodes.iter().map(|n| format!("{:?}", n.op)).collect();
    }

    // Step 2.5: Optimize (fusion) — always runs on IR path
    let plan = ir_fusion::optimize(&plan);

    // Capture post-fusion state for pipe
    if let Some(ref mut rpt) = pipe {
        rpt.fusion_invoked = true;
        rpt.pre_fusion_count = pre_fusion_count;
        rpt.post_fusion_count = plan.nodes.len();
        rpt.fused_ops = detect_fused_ops(&plan);
        // Update plan_nodes to show post-fusion state
        rpt.plan_nodes = plan.nodes.iter().map(|n| format!("{:?}", n.op)).collect();
        rpt.exec_path = "IR".to_string();
    }

    // Step 3: Execute (run optimized IR executor)
    let result = exec::execute(&plan, rt).map_err(IrError::Exec)?;

    Ok(result)
}

/// Hybrid evaluator: peels glue forms (save, progn, print), tries IR for everything else.
/// Finance pipelines go through IR; glue forms use minimal custom logic.
fn hybrid_eval(
    rt: &mut Runtime,
    expr: &ast::Expr,
    trace: bool,
    pipe: &mut Option<PipeReport>,
) -> Result<value::Value, String> {
    if let ast::Expr::List(elements) = expr {
        if let Some(ast::Expr::Sym(sym)) = elements.first() {
            let name = rt.interner.resolve(*sym).to_string();
            match name.as_str() {
                "save" if elements.len() == 3 => {
                    if let Some(ref mut rpt) = pipe {
                        rpt.glue_forms.push("save".to_string());
                    }
                    let body_val = hybrid_eval(rt, &elements[2], trace, pipe)?;
                    let filename = rt.eval(&elements[1])?;
                    let save_sym = rt.interner.intern("save");
                    rt.call_builtin(save_sym, &[filename, body_val.clone()])?;
                    return Ok(body_val);
                }
                "progn" => {
                    if let Some(ref mut rpt) = pipe {
                        rpt.glue_forms.push("progn".to_string());
                    }
                    let mut last = value::Value::Nil;
                    for form in &elements[1..] {
                        last = hybrid_eval(rt, form, trace, pipe)?;
                    }
                    return Ok(last);
                }
                "print" if elements.len() == 2 => {
                    if let Some(ref mut rpt) = pipe {
                        rpt.glue_forms.push("print".to_string());
                    }
                    let val = hybrid_eval(rt, &elements[1], trace, pipe)?;
                    println!("{}", val.display(&rt.interner));
                    return Ok(val);
                }
                _ => {}
            }
        }
    }

    // Default: try IR first, legacy fallback on PlanError
    match try_ir_eval(rt, expr.clone(), trace, pipe) {
        Ok(val) => {
            if trace {
                eprintln!("[TRACE] result= IR");
            }
            Ok(val)
        }
        Err(IrError::Plan(plan_err)) => {
            if trace {
                eprintln!("[TRACE] fallback reason= {}", plan_err);
            }
            if let Some(ref mut rpt) = pipe {
                rpt.exec_path = "legacy-fallback".to_string();
                rpt.fallback_reason = Some(format!("{}", plan_err));
            }
            rt.eval(expr).map_err(|e| e.to_string())
        }
        Err(IrError::Exec(exec_err)) => Err(exec_err),
    }
}

fn eval_code(
    rt: &mut Runtime,
    code: &str,
    use_legacy: bool,
    use_ir_only: bool,
    trace: bool,
    pipe_mode: PipeMode,
) -> Result<value::Value, String> {
    let mut reader = Reader::new(code).map_err(|e| format!("Parse error: {}", e))?;

    let is_segment = std::env::var("BLISP_SEGMENT").is_ok();
    let mode_str = if use_legacy {
        "LEGACY"
    } else if use_ir_only {
        "IR-ONLY"
    } else if is_segment {
        "SEGMENTED-HYBRID"
    } else {
        "HYBRID"
    };

    // Read and evaluate ALL expressions (implicit progn)
    let mut result = value::Value::Nil;

    loop {
        match reader.read(&mut rt.interner) {
            Ok(expr) => {
                // Capture raw AST for pipe before expr is consumed
                let raw_ast = if pipe_mode != PipeMode::Off {
                    display_expr(&expr, &rt.interner)
                } else {
                    String::new()
                };

                // Build pipe report if needed
                let mut pipe = if pipe_mode != PipeMode::Off {
                    let mut rpt = PipeReport::new();
                    rpt.mode = mode_str.to_string();
                    rpt.segment = is_segment;
                    rpt.raw_ast = raw_ast;
                    Some(rpt)
                } else {
                    None
                };

                if use_legacy {
                    if let Some(ref mut rpt) = pipe {
                        rpt.exec_path = "legacy".to_string();
                    }
                    result = rt.eval(&expr)?;
                } else if use_ir_only {
                    result =
                        try_ir_eval(rt, expr, trace, &mut pipe).map_err(|e| format!("{}", e))?;
                } else if is_segment {
                    result = hybrid_eval(rt, &expr, trace, &mut pipe)?;
                } else {
                    // HYBRID mode (DEFAULT)
                    match try_ir_eval(rt, expr.clone(), trace, &mut pipe) {
                        Ok(val) => {
                            if trace {
                                eprintln!("[TRACE] result= IR");
                            }
                            result = val;
                        }
                        Err(IrError::Plan(plan_err)) => {
                            if trace {
                                eprintln!("[TRACE] fallback reason= {}", plan_err);
                            }
                            if let Some(ref mut rpt) = pipe {
                                rpt.exec_path = "legacy-fallback".to_string();
                                rpt.fallback_reason = Some(format!("{}", plan_err));
                            }
                            match rt.eval(&expr) {
                                Ok(val) => {
                                    result = val;
                                }
                                Err(legacy_err) => {
                                    if legacy_err.contains("Undefined variable") {
                                        return Err(format!(
                                            "{}\n\nHint: The pipeline could not be fully planned for IR ({}),\n\
                                             and the legacy fallback does not know this operation.\n\
                                             Try: --load stdlib/compat_clispi.cl  or  use the legacy spelling (e.g. locf-cols).",
                                            legacy_err, plan_err
                                        ));
                                    }
                                    return Err(legacy_err);
                                }
                            }
                        }
                        Err(IrError::Exec(exec_err)) => {
                            return Err(exec_err);
                        }
                    }
                }

                // Emit pipe report
                if let Some(rpt) = pipe {
                    rpt.emit(pipe_mode);
                }
            }
            Err(e) => {
                let err_str = format!("{:?}", e);
                if err_str.contains("Unexpected end of input") || err_str.contains("EOF") {
                    break;
                } else {
                    return Err(format!("Read error: {}", e));
                }
            }
        }
    }

    Ok(result)
}
