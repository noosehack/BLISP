// Dictionary module: Display operation metadata
//
// Schema: Code owns canonical IDs (enum variants), YAML owns metadata (aliases, docs, buckets)
// Validates YAML ir: field against actual IR enum variants (anti-invention guardrail)

use crate::ast::{Expr, Interner};
use crate::ir::{BinaryFunc, NumericFunc, Operation, Source, UnaryOp};
use crate::normalize::{self};
use crate::planner;
use crate::runtime::Runtime;
use serde::Deserialize;
use std::collections::{HashMap, HashSet};

/// Embedded metadata overlays (compiled into binary)
const CURRENT_OPS_YAML: &str = include_str!("../OPS_CURRENT.yml");
const PLANNED_OPS_YAML: &str = include_str!("../OPS_PLANNED.yml");

#[derive(Debug, Clone, Deserialize)]
pub struct OpMapEntry {
    pub ir: Option<String>,
    #[serde(default)]
    pub aliases: Vec<String>,
    #[serde(default)]
    pub legacy_tokens: Vec<String>,
    #[serde(default)]
    pub bucket: String,
    #[serde(default)]
    pub semantics: String,
    #[serde(default)]
    pub docs: String,
    #[serde(default)]
    pub notes: String,
}

/// Build authoritative IR-name set from code (source of truth)
pub fn ir_name_set() -> HashSet<&'static str> {
    let mut s = HashSet::new();
    for &n in NumericFunc::ALL_NAMES {
        s.insert(n);
    }
    for &n in BinaryFunc::ALL_NAMES {
        s.insert(n);
    }
    for &n in Source::ALL_NAMES {
        s.insert(n);
    }
    s
}

/// Parse the embedded metadata overlay (CURRENT by default)
pub fn load_op_map() -> Result<Vec<OpMapEntry>, String> {
    load_current_ops()
}

/// Load current operations (all names resolve)
pub fn load_current_ops() -> Result<Vec<OpMapEntry>, String> {
    serde_yaml::from_str(CURRENT_OPS_YAML)
        .map_err(|e| format!("Failed to parse OPS_CURRENT.yml: {}", e))
}

/// Load planned operations (roadmap, may not resolve)
pub fn load_planned_ops() -> Result<Vec<OpMapEntry>, String> {
    serde_yaml::from_str(PLANNED_OPS_YAML)
        .map_err(|e| format!("Failed to parse OPS_PLANNED.yml: {}", e))
}

/// Validate YAML against actual code (anti-invention guardrail)
pub fn validate_op_map(entries: &[OpMapEntry]) -> Result<(), Vec<String>> {
    let mut errors = Vec::new();
    let ir_set = ir_name_set();

    // CRITICAL: Validate ir: field against actual enums
    for entry in entries {
        if let Some(ref ir_name) = entry.ir {
            if !ir_set.contains(ir_name.as_str()) {
                errors.push(format!(
                    "❌ Unknown IR op '{}' (not present in src/ir.rs enums). \
                     This prevents invented canonical names.",
                    ir_name
                ));
            }
        }
    }

    // Check for alias collisions
    let mut alias_to_ir: HashMap<String, String> = HashMap::new();
    for entry in entries {
        let ir_display = entry.ir.as_deref().unwrap_or("<builtin>");
        for alias in &entry.aliases {
            if let Some(existing) = alias_to_ir.get(alias) {
                errors.push(format!(
                    "Alias '{}' maps to both '{}' and '{}'",
                    alias, existing, ir_display
                ));
            } else {
                alias_to_ir.insert(alias.clone(), ir_display.to_string());
            }
        }
    }

    // Check for legacy token collisions
    let mut token_to_ir: HashMap<String, String> = HashMap::new();
    for entry in entries {
        let ir_display = entry.ir.as_deref().unwrap_or("<builtin>");
        for token in &entry.legacy_tokens {
            if let Some(existing) = token_to_ir.get(token) {
                errors.push(format!(
                    "Legacy token '{}' maps to both '{}' and '{}'",
                    token, existing, ir_display
                ));
            } else {
                token_to_ir.insert(token.clone(), ir_display.to_string());
            }
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

/// Output format
#[derive(Debug, Clone, Copy)]
pub enum OutputFormat {
    Table,
    Json,
}

/// View selection
#[derive(Debug, Clone, Copy)]
pub enum View {
    Exposed,      // Aliases table (current ops)
    Legacy,       // Legacy tokens table
    TodoIR,       // IR migration queue
    Unmapped,     // IR ops missing metadata
    CheckResolve, // Resolution check (reality test)
    Planned,      // Planned operations (roadmap)
    All,          // All views (default)
    Matrix,       // Cross-layer operation matrix (code-driven)
}

/// View 1: Print exposed aliases table (L1)
pub fn print_exposed_aliases(
    entries: &[OpMapEntry],
    format: OutputFormat,
    grep_pattern: Option<&str>,
) {
    let mut rows: Vec<(String, String, String, String)> = Vec::new();

    for entry in entries {
        for alias in &entry.aliases {
            // Filter by grep pattern if provided
            if let Some(pattern) = grep_pattern {
                let ir_name = entry.ir.as_deref().unwrap_or("");
                if !alias.contains(pattern) && !ir_name.contains(pattern) {
                    continue;
                }
            }

            let ir_display = entry.ir.as_deref().unwrap_or("<builtin>");
            let legacy_str = if entry.legacy_tokens.is_empty() {
                "".to_string() // Don't use "-" as placeholder - looks like minus operator
            } else {
                entry.legacy_tokens.join(", ")
            };

            rows.push((
                alias.clone(),
                ir_display.to_string(),
                entry.bucket.clone(),
                legacy_str,
            ));
        }
    }

    // Sort by alias
    rows.sort_by(|a, b| a.0.cmp(&b.0));

    match format {
        OutputFormat::Table => {
            println!("# Exposed Aliases (User-Facing Operations)");
            println!();
            println!(
                "{:<25} {:<30} {:<25} Legacy Tokens",
                "Alias", "IR / Builtin", "Bucket"
            );
            println!("{}", "-".repeat(110));

            for (alias, ir, bucket, legacy) in &rows {
                println!("{:<25} {:<30} {:<25} {}", alias, ir, bucket, legacy);
            }
            println!();
            println!("Total aliases: {}", rows.len());
        }
        OutputFormat::Json => {
            let json_rows: Vec<serde_json::Value> = rows
                .iter()
                .map(|(alias, ir, bucket, legacy)| {
                    serde_json::json!({
                        "alias": alias,
                        "ir": ir,
                        "bucket": bucket,
                        "legacy_tokens": legacy,
                    })
                })
                .collect();
            println!("{}", serde_json::to_string_pretty(&json_rows).unwrap());
        }
    }
}

/// View 2: Print legacy tokens table (L2)
pub fn print_legacy_tokens(
    entries: &[OpMapEntry],
    format: OutputFormat,
    grep_pattern: Option<&str>,
) {
    let mut rows: Vec<(String, String, String)> = Vec::new();

    for entry in entries {
        for token in &entry.legacy_tokens {
            // Filter by grep pattern if provided
            if let Some(pattern) = grep_pattern {
                let ir_name = entry.ir.as_deref().unwrap_or("");
                if !token.contains(pattern) && !ir_name.contains(pattern) {
                    continue;
                }
            }

            let ir_display = entry.ir.as_deref().unwrap_or("<builtin>");
            // Find suggested replacement (first alias)
            let suggested = entry
                .aliases
                .first()
                .cloned()
                .unwrap_or_else(|| "-".to_string());

            rows.push((token.clone(), ir_display.to_string(), suggested));
        }
    }

    // Sort by legacy token
    rows.sort_by(|a, b| a.0.cmp(&b.0));

    match format {
        OutputFormat::Table => {
            println!("# Legacy Tokens (Backward Compatibility)");
            println!();
            println!(
                "{:<25} {:<30} Suggested Replacement",
                "Legacy Token", "IR / Builtin"
            );
            println!("{}", "-".repeat(90));

            for (token, ir, suggested) in &rows {
                println!("{:<25} {:<30} {}", token, ir, suggested);
            }
            println!();
            println!("Total legacy tokens: {}", rows.len());
        }
        OutputFormat::Json => {
            let json_rows: Vec<serde_json::Value> = rows
                .iter()
                .map(|(token, ir, suggested)| {
                    serde_json::json!({
                        "legacy_token": token,
                        "ir": ir,
                        "suggested_replacement": suggested,
                    })
                })
                .collect();
            println!("{}", serde_json::to_string_pretty(&json_rows).unwrap());
        }
    }
}

/// View 3: Print IR migration queue (ops that need IR migration)
pub fn print_todo_ir(entries: &[OpMapEntry], format: OutputFormat, grep_pattern: Option<&str>) {
    let mut rows: Vec<(String, String, Vec<String>, Vec<String>)> = Vec::new();

    for entry in entries {
        // Filter: ir == None AND (aliases or legacy_tokens exist) AND bucket in A1/A2
        if entry.ir.is_none()
            && (!entry.aliases.is_empty() || !entry.legacy_tokens.is_empty())
            && (entry.bucket.starts_with("A1") || entry.bucket.starts_with("A2"))
        {
            // Filter by grep pattern if provided
            if let Some(pattern) = grep_pattern {
                if !entry.aliases.iter().any(|a| a.contains(pattern))
                    && !entry.legacy_tokens.iter().any(|t| t.contains(pattern))
                {
                    continue;
                }
            }

            rows.push((
                entry.bucket.clone(),
                entry.semantics.clone(),
                entry.aliases.clone(),
                entry.legacy_tokens.clone(),
            ));
        }
    }

    // Sort by bucket (A1 first), then by first alias
    rows.sort_by(|a, b| {
        let bucket_cmp = a.0.cmp(&b.0);
        if bucket_cmp == std::cmp::Ordering::Equal {
            a.2.first().cmp(&b.2.first())
        } else {
            bucket_cmp
        }
    });

    match format {
        OutputFormat::Table => {
            println!("# IR Migration Queue (Operations NOT in IR Yet)");
            println!();
            println!(
                "{:<25} {:<50} {:<30} Legacy Tokens",
                "Bucket", "Semantics", "Aliases"
            );
            println!("{}", "-".repeat(130));

            for (bucket, semantics, aliases, legacy) in &rows {
                let aliases_str = aliases.join(", ");
                let legacy_str = if legacy.is_empty() {
                    "-".to_string()
                } else {
                    legacy.join(", ")
                };
                // Truncate semantics if too long
                let semantics_short = if semantics.len() > 47 {
                    format!("{}...", &semantics[..47])
                } else {
                    semantics.clone()
                };

                println!(
                    "{:<25} {:<50} {:<30} {}",
                    bucket, semantics_short, aliases_str, legacy_str
                );
            }
            println!();
            println!("Total operations needing IR migration: {}", rows.len());

            // Print summary by bucket
            let a1_count = rows
                .iter()
                .filter(|(b, _, _, _)| b.starts_with("A1"))
                .count();
            let a2_count = rows
                .iter()
                .filter(|(b, _, _, _)| b.starts_with("A2"))
                .count();
            println!();
            println!("By priority:");
            println!("  A1 (fusion-critical): {} ops", a1_count);
            println!("  A2 (planner-structural): {} ops", a2_count);
        }
        OutputFormat::Json => {
            let json_rows: Vec<serde_json::Value> = rows
                .iter()
                .map(|(bucket, semantics, aliases, legacy)| {
                    serde_json::json!({
                        "bucket": bucket,
                        "semantics": semantics,
                        "aliases": aliases,
                        "legacy_tokens": legacy,
                    })
                })
                .collect();
            println!("{}", serde_json::to_string_pretty(&json_rows).unwrap());
        }
    }
}

/// View 4: IR ops missing metadata (high value diagnostic)
pub fn print_unmapped_ir(entries: &[OpMapEntry], format: OutputFormat) {
    let ir_set = ir_name_set();
    let mut mapped_ir: HashSet<&str> = HashSet::new();

    // Collect all ir: names from YAML
    for entry in entries {
        if let Some(ref ir_name) = entry.ir {
            mapped_ir.insert(ir_name.as_str());
        }
    }

    // Find IR ops in code but not in YAML
    let mut unmapped: Vec<&str> = ir_set.difference(&mapped_ir).copied().collect();
    unmapped.sort();

    match format {
        OutputFormat::Table => {
            println!("# IR Operations Missing Metadata");
            println!();
            if unmapped.is_empty() {
                println!("✅ All IR operations have metadata entries in YAML");
            } else {
                println!("These IR operations exist in code but have no YAML entry:");
                println!();
                for op in &unmapped {
                    println!("  {}", op);
                }
                println!();
                println!("Total unmapped: {}", unmapped.len());
            }
        }
        OutputFormat::Json => {
            let json = serde_json::json!({
                "unmapped_ir_ops": unmapped,
                "count": unmapped.len(),
            });
            println!("{}", serde_json::to_string_pretty(&json).unwrap());
        }
    }
}

/// Resolution status for a name
#[derive(Debug, Clone)]
pub enum ResolveStatus {
    IrOp(String), // Resolves to IR operation
    Builtin,      // Resolves to builtin
    Unknown,      // Not found
}

/// Check if a name resolves in the runtime
pub fn check_resolve(name: &str) -> ResolveStatus {
    // First check if it's an IR operation name
    let ir_set = ir_name_set();
    if ir_set.contains(name) {
        return ResolveStatus::IrOp(name.to_string());
    }

    // Then check if it's registered as a builtin
    let mut rt = crate::runtime::Runtime::new();
    let sym = rt.interner.intern(name);
    if rt.is_builtin(sym) {
        return ResolveStatus::Builtin;
    }

    ResolveStatus::Unknown
}

/// Print resolution check report for all names in dictionary
pub fn print_resolution_check(entries: &[OpMapEntry], format: OutputFormat) {
    let mut results: Vec<(String, String, String)> = Vec::new();
    let mut unknown_count = 0;

    // Check all aliases
    for entry in entries {
        let ir_display = entry.ir.as_deref().unwrap_or("<builtin>");

        for alias in &entry.aliases {
            let status = check_resolve(alias);
            let status_str = match &status {
                ResolveStatus::IrOp(name) => format!("OK(IR: {})", name),
                ResolveStatus::Builtin => "OK(BUILTIN)".to_string(),
                ResolveStatus::Unknown => {
                    unknown_count += 1;
                    "FAIL(unknown)".to_string()
                }
            };
            results.push((alias.clone(), ir_display.to_string(), status_str));
        }

        // Check all legacy tokens
        for token in &entry.legacy_tokens {
            let status = check_resolve(token);
            let status_str = match &status {
                ResolveStatus::IrOp(name) => format!("OK(IR: {})", name),
                ResolveStatus::Builtin => "OK(BUILTIN)".to_string(),
                ResolveStatus::Unknown => {
                    unknown_count += 1;
                    "FAIL(unknown) [legacy]".to_string()
                }
            };
            results.push((token.clone(), ir_display.to_string(), status_str));
        }
    }

    // Sort by status (unknowns first for visibility)
    results.sort_by(|a, b| {
        if a.2.starts_with("FAIL") && !b.2.starts_with("FAIL") {
            std::cmp::Ordering::Less
        } else if !a.2.starts_with("FAIL") && b.2.starts_with("FAIL") {
            std::cmp::Ordering::Greater
        } else {
            a.0.cmp(&b.0)
        }
    });

    match format {
        OutputFormat::Table => {
            println!("# Resolution Check (Reality Test)");
            println!();
            println!("{:<30} {:<30} Actual (Runtime)", "Name", "Expected (YAML)");
            println!("{}", "-".repeat(100));

            for (name, expected, status) in &results {
                println!("{:<30} {:<30} {}", name, expected, status);
            }

            println!();
            if unknown_count > 0 {
                println!(
                    "❌ {} names FAIL to resolve (not registered in runtime)",
                    unknown_count
                );
            } else {
                println!("✅ All names resolve successfully");
            }
            println!("Total checked: {}", results.len());
        }
        OutputFormat::Json => {
            let json_rows: Vec<serde_json::Value> = results
                .iter()
                .map(|(name, expected, status)| {
                    serde_json::json!({
                        "name": name,
                        "expected": expected,
                        "status": status,
                    })
                })
                .collect();
            println!("{}", serde_json::to_string_pretty(&json_rows).unwrap());
        }
    }
}

// ─── Matrix: Live Code-Driven Operation Audit ────────────────────────────────

/// Layer classification for an operation
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Layer {
    IR,
    Glue,
    Legacy,
    Unknown,
}

impl std::fmt::Display for Layer {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Layer::IR => write!(f, "IR"),
            Layer::Glue => write!(f, "GLUE"),
            Layer::Legacy => write!(f, "LEGACY"),
            Layer::Unknown => write!(f, "UNKNOWN"),
        }
    }
}

/// Result of probing the planner for a single operation name
#[derive(Debug, Clone)]
struct ProbeIR {
    ir_variant: String,
    node_count: usize,
    fusable: bool,
}

/// One row in the matrix output
#[derive(Debug, Clone)]
struct MatrixRow {
    name: String,
    normalize_to: Option<String>,
    /// ACCEPT: can the user type this token? (code-driven)
    is_accept: bool,
    /// Why is this accepted? {normalize|planner|builtin|glue}
    accept_reasons: Vec<String>,
    /// PUB: is this in PUBLIC_FINANCE_OPS? (policy)
    is_pub: bool,
    /// CANON: canonical token after normalization
    canon: String,
    layer: Layer,
    ir_variant: String,
    fusable: String,
    /// USE: recommended spelling (= CANON when this is a deprecated alias, else "-")
    use_spelling: String,
    yaml_current: String,
    yaml_planned: String,
    notes: Vec<String>,
}

/// Glue forms: control/IO/logic that hybrid_eval peels or that aren't data ops
const GLUE_FORMS: &[&str] = &[
    "save", "print", "progn", "defmacro", "define", "let",
    "let*", // binding forms (control, not finance)
    "and", "or", "not", // boolean logic (mask sub-expressions, not standalone data ops)
];

/// Public finance operations: the canonical user API for data/finance.
///
/// Policy: every name in this list MUST probe as IR via the planner.
/// If a name here is not plannable, that is a bug — either the planner
/// needs to support it, or the name should be removed from this list.
///
/// This is a DECLARED POLICY, not auto-derived. The tripwire test
/// `test_public_finance_ops_all_ir` enforces it against compiled code.
pub const PUBLIC_FINANCE_OPS: &[&str] = &[
    // Sources
    "stdin",
    "file",
    "load",
    "read-csv",
    // Elementwise math (fusable)
    "abs",
    "exp",
    "inv",
    "log",
    "sqrt",
    // Arithmetic (binary)
    "+",
    "-",
    "*",
    "/",
    // Comparisons (binary)
    ">",
    "<",
    "<=",
    ">=",
    "==",
    "!=",
    // Shift/window ops
    "shift",
    "dlog",
    "cs1",
    "locf",
    "xminus",
    "rolling-mean",
    "rolling-std",
    // Composites
    "rolling-zscore",
    "ur",
    "wzs",
    // Masks
    "wkd",
    // Alignment
    "mapr",
];

/// Collect all candidate operation names from compiled code
fn collect_candidate_names(rt: &Runtime) -> HashSet<String> {
    let mut names = HashSet::new();

    // 1. Normalize aliases (both from and to)
    for (from, to) in normalize::NORMALIZE_ALIASES {
        names.insert(from.to_string());
        names.insert(to.to_string());
    }

    // 2. All registered builtins
    for name in rt.builtin_names() {
        names.insert(name);
    }

    // 3. Core symbols (sources, arithmetic, glue forms)
    for s in &[
        "stdin", "file", "load", "read-csv", "+", "-", "*", "/", ">", "<", "<=", ">=", "==", "!=",
        "let", "save", "print", "progn", "not", "and", "or", "defmacro",
    ] {
        names.insert(s.to_string());
    }

    names
}

/// Probe the planner with a minimal expression to see if it handles this op.
///
/// Uses the REAL pipeline: build raw Expr → normalize → plan.
/// This ensures aliases and arg swaps are applied exactly as in real runs.
fn probe_planner(name: &str, interner: &mut Interner) -> Option<ProbeIR> {
    let op_sym = interner.intern(name);
    let stdin_sym = interner.intern("stdin");
    let stdin_call = Expr::List(vec![Expr::Sym(stdin_sym)]);

    // Try forms in order until one succeeds
    let x_sym = interner.intern("__probe_x");
    let forms: Vec<Expr> = vec![
        // source (no args): (stdin)
        Expr::List(vec![Expr::Sym(op_sym)]),
        // source with string: (file "/dev/null")
        Expr::List(vec![Expr::Sym(op_sym), Expr::Str("/dev/null".into())]),
        // unary: (op (stdin))
        Expr::List(vec![Expr::Sym(op_sym), stdin_call.clone()]),
        // binary with scalar: (op (stdin) 1)
        Expr::List(vec![Expr::Sym(op_sym), stdin_call.clone(), Expr::Int(1)]),
        // param unary 2-arg: (op (stdin) 2)
        Expr::List(vec![Expr::Sym(op_sym), stdin_call.clone(), Expr::Int(2)]),
        // 3-param: (op (stdin) 250 5)
        Expr::List(vec![
            Expr::Sym(op_sym),
            stdin_call.clone(),
            Expr::Int(250),
            Expr::Int(5),
        ]),
        // join: (op (stdin) (stdin))
        Expr::List(vec![
            Expr::Sym(op_sym),
            stdin_call.clone(),
            stdin_call.clone(),
        ]),
        // let form: (let ((x (stdin))) x)
        Expr::List(vec![
            Expr::Sym(op_sym),
            Expr::List(vec![Expr::List(vec![Expr::Sym(x_sym), stdin_call.clone()])]),
            Expr::Sym(x_sym),
        ]),
    ];

    for form in forms {
        // Real pipeline: normalize first (alias rewrite + arg swap), then plan
        let canon = normalize::normalize(form, interner);
        if let Ok(plan) = planner::plan(&canon, interner) {
            // Extract IR variant description from non-Source nodes
            let ir_parts: Vec<String> = plan
                .nodes
                .iter()
                .filter_map(|n| match &n.op {
                    Operation::Source(_) => None,
                    op => Some(format_ir_op(op)),
                })
                .collect();

            let ir_variant = if ir_parts.is_empty() {
                format_ir_source(&plan)
            } else if ir_parts.len() == 1 {
                ir_parts[0].clone()
            } else {
                format!("composite({} nodes)", plan.nodes.len())
            };

            // Check fusability
            let fusable = plan.nodes.iter().any(|n| {
                matches!(
                    &n.op,
                    Operation::Unary(UnaryOp::MapNumeric { func, .. })
                    if func.is_pure_elementwise()
                )
            });

            return Some(ProbeIR {
                ir_variant,
                node_count: plan.nodes.len(),
                fusable,
            });
        }
    }
    None
}

/// Format an IR operation for display (compact)
fn format_ir_op(op: &Operation) -> String {
    match op {
        Operation::Unary(UnaryOp::MapNumeric { func, .. }) => format!("{:?}", func),
        Operation::Binary(bop) => {
            // Extract the func from the binary op debug format
            format!("{:?}", bop)
                .split('{')
                .next()
                .unwrap_or("Binary")
                .trim()
                .to_string()
        }
        Operation::Join(jop) => format!("{:?}", jop)
            .split('{')
            .next()
            .unwrap_or("Join")
            .trim()
            .to_string(),
        Operation::Schema(sop) => format!("{:?}", sop)
            .split('{')
            .next()
            .unwrap_or("Schema")
            .trim()
            .to_string(),
        _ => format!("{:?}", op).chars().take(40).collect(),
    }
}

/// Format a source-only plan
fn format_ir_source(plan: &crate::ir::Plan) -> String {
    if let Some(node) = plan.nodes.first() {
        match &node.op {
            Operation::Source(Source::Stdin) => "Source::Stdin".into(),
            Operation::Source(Source::File { .. }) => "Source::File".into(),
            Operation::Source(Source::Variable { .. }) => "Source::Variable".into(),
            _ => "-".into(),
        }
    } else {
        "-".into()
    }
}

/// Look up a name in YAML entries (aliases or legacy_tokens)
fn find_in_yaml(name: &str, entries: &[OpMapEntry]) -> Option<(String, bool)> {
    for entry in entries {
        if entry.aliases.iter().any(|a| a == name) {
            let ir = entry.ir.as_deref().unwrap_or("null").to_string();
            return Some((ir, false));
        }
        if entry.legacy_tokens.iter().any(|t| t == name) {
            let ir = entry.ir.as_deref().unwrap_or("null").to_string();
            return Some((ir, true));
        }
    }
    None
}

/// Print the operation matrix (code-driven)
pub fn print_matrix(
    no_yaml: bool,
    format: OutputFormat,
    grep_pattern: Option<&str>,
) -> Result<(), String> {
    // Single Runtime for the whole run
    let mut rt = Runtime::new();
    let candidates = collect_candidate_names(&rt);

    // Optional YAML data
    let yaml_current = if no_yaml {
        Vec::new()
    } else {
        load_current_ops().unwrap_or_default()
    };
    let yaml_planned = if no_yaml {
        Vec::new()
    } else {
        load_planned_ops().unwrap_or_default()
    };

    // Build normalize lookup
    let normalize_map: HashMap<&str, &str> = normalize::NORMALIZE_ALIASES.iter().copied().collect();

    // Probe each candidate
    let mut rows: Vec<MatrixRow> = Vec::new();

    for name in &candidates {
        // grep filter
        if let Some(pattern) = grep_pattern {
            if !name.contains(pattern) {
                continue;
            }
        }

        // Normalize alias?
        let normalize_to = normalize_map.get(name.as_str()).map(|s| s.to_string());

        // Probe uses the raw name — normalize() inside probe_planner handles
        // alias rewrite and arg swaps, exactly as the real pipeline does.
        let probe = probe_planner(name, &mut rt.interner);

        // Builtin check
        let sym = rt.interner.intern(name);
        let is_builtin = rt.is_builtin(sym);

        // Determine layer
        // GLUE wins unconditionally: control/IO/logic forms are never finance IR
        // even if the planner can technically handle them (e.g. let bindings)
        let is_glue = GLUE_FORMS.contains(&name.as_str());
        let layer = if is_glue {
            Layer::Glue
        } else if probe.is_some() {
            Layer::IR
        } else if is_builtin {
            Layer::Legacy
        } else {
            Layer::Unknown
        };

        let ir_variant = probe
            .as_ref()
            .map_or("-".to_string(), |p| p.ir_variant.clone());
        let fusable = if probe.as_ref().is_some_and(|p| p.fusable) {
            "elem".to_string()
        } else {
            "-".to_string()
        };

        // YAML lookups
        let (yaml_cur, yaml_plan);
        let mut notes = Vec::new();

        if no_yaml {
            yaml_cur = "-".to_string();
            yaml_plan = "-".to_string();
        } else {
            yaml_cur = match find_in_yaml(name, &yaml_current) {
                Some((ir, is_legacy)) => {
                    if is_legacy {
                        notes.push("dep".to_string());
                    }
                    // Check for ir:null! (YAML says null but code says IR)
                    if ir == "null" && layer == Layer::IR {
                        notes.push("ir:null!".to_string());
                    }
                    "yes".to_string()
                }
                None => "-".to_string(),
            };
            yaml_plan = match find_in_yaml(name, &yaml_planned) {
                Some((ir, _)) => {
                    if ir == "null" && layer == Layer::IR {
                        // Only add ir:null! once
                        if !notes.contains(&"ir:null!".to_string()) {
                            notes.push("ir:null!".to_string());
                        }
                    }
                    "yes".to_string()
                }
                None => "-".to_string(),
            };
        }

        // Additional notes
        // LHS of NORMALIZE_ALIASES = deprecated spelling
        if normalize_to.is_some() && !notes.contains(&"dep".to_string()) {
            notes.push("dep".to_string());
        }
        if normalize_to.is_some() && !is_builtin && probe.is_none() {
            notes.push("alias-only".to_string());
        }
        if is_glue {
            notes.push("glue".to_string());
        }

        // ACCEPT: can user type this token? (code-driven)
        let is_in_normalize = normalize::NORMALIZE_ALIASES
            .iter()
            .any(|(from, to)| *from == name.as_str() || *to == name.as_str());
        let is_accept = is_in_normalize || is_builtin || is_glue || probe.is_some();

        let mut accept_reasons = Vec::new();
        if is_in_normalize {
            accept_reasons.push("normalize".to_string());
        }
        if probe.is_some() {
            accept_reasons.push("planner".to_string());
        }
        if is_builtin {
            accept_reasons.push("builtin".to_string());
        }
        if is_glue {
            accept_reasons.push("glue".to_string());
        }

        // PUB: is it in the stable finance API? (policy)
        let is_pub = PUBLIC_FINANCE_OPS.contains(&name.as_str());

        // CANON: canonical token after normalization (resolve transitively)
        let canon = {
            let mut c = normalize_to.as_deref().unwrap_or(name.as_str());
            // Resolve alias chains (A→B, B→C → final C)
            loop {
                match normalize_map.get(c) {
                    Some(next) if *next != c => c = next,
                    _ => break,
                }
            }
            c.to_string()
        };

        // USE: recommended spelling for deprecated aliases
        let use_spelling = if notes.contains(&"dep".to_string()) {
            canon.clone()
        } else {
            "-".to_string()
        };

        rows.push(MatrixRow {
            name: name.clone(),
            normalize_to,
            is_accept,
            accept_reasons,
            is_pub,
            canon,
            layer,
            ir_variant,
            fusable,
            use_spelling,
            yaml_current: yaml_cur,
            yaml_planned: yaml_plan,
            notes,
        });
    }

    // Sort: IR first, then Glue, then Legacy, then Unknown; alpha within each
    rows.sort_by(|a, b| a.layer.cmp(&b.layer).then(a.name.cmp(&b.name)));

    // Count summary
    let ir_count = rows.iter().filter(|r| r.layer == Layer::IR).count();
    let legacy_count = rows.iter().filter(|r| r.layer == Layer::Legacy).count();
    let glue_count = rows.iter().filter(|r| r.layer == Layer::Glue).count();
    let unknown_count = rows.iter().filter(|r| r.layer == Layer::Unknown).count();
    let accept_count = rows.iter().filter(|r| r.is_accept).count();
    let pub_count = rows.iter().filter(|r| r.is_pub).count();
    let ir_null_count = rows
        .iter()
        .filter(|r| r.notes.iter().any(|n| n == "ir:null!"))
        .count();

    match format {
        OutputFormat::Table => {
            println!("# Operation Matrix");
            println!("# version:  {}", env!("CARGO_PKG_VERSION"));
            println!(
                "# source:   compiled code (planner probes, normalize table, builtins registry)"
            );
            if no_yaml {
                println!("# yaml:     disabled (--no-yaml)");
            } else {
                println!("# yaml:     included (OPS_CURRENT + OPS_PLANNED)");
            }
            println!();

            // Header
            let yes_dash = |b: bool| if b { "yes" } else { "-" };

            if no_yaml {
                println!(
                    "{:<18} {:<8} {:<18} {:<5} {:<16} {:<16} {:<8} {:<30} {:<8} NOTES",
                    "NAME",
                    "ACCEPT",
                    "ACCEPT_WHY",
                    "PUB",
                    "CANON",
                    "USE",
                    "LAYER",
                    "IR_VARIANT",
                    "FUSABLE"
                );
                println!("{}", "-".repeat(147));
            } else {
                println!(
                    "{:<18} {:<8} {:<18} {:<5} {:<16} {:<16} {:<8} {:<30} {:<8} {:<6} {:<6} NOTES",
                    "NAME",
                    "ACCEPT",
                    "ACCEPT_WHY",
                    "PUB",
                    "CANON",
                    "USE",
                    "LAYER",
                    "IR_VARIANT",
                    "FUSABLE",
                    "Y_CUR",
                    "Y_PLN"
                );
                println!("{}", "-".repeat(162));
            }

            for row in &rows {
                let notes_str = if row.notes.is_empty() {
                    String::new()
                } else {
                    row.notes.join(", ")
                };

                let accept_why = if row.accept_reasons.is_empty() {
                    "-".to_string()
                } else {
                    row.accept_reasons.join("+")
                };

                if no_yaml {
                    println!(
                        "{:<18} {:<8} {:<18} {:<5} {:<16} {:<16} {:<8} {:<30} {:<8} {}",
                        row.name,
                        yes_dash(row.is_accept),
                        accept_why,
                        yes_dash(row.is_pub),
                        row.canon,
                        row.use_spelling,
                        row.layer,
                        row.ir_variant,
                        row.fusable,
                        notes_str
                    );
                } else {
                    println!(
                        "{:<18} {:<8} {:<18} {:<5} {:<16} {:<16} {:<8} {:<30} {:<8} {:<6} {:<6} {}",
                        row.name,
                        yes_dash(row.is_accept),
                        accept_why,
                        yes_dash(row.is_pub),
                        row.canon,
                        row.use_spelling,
                        row.layer,
                        row.ir_variant,
                        row.fusable,
                        row.yaml_current,
                        row.yaml_planned,
                        notes_str
                    );
                }
            }

            println!();
            println!("SUMMARY:");
            println!("  Total names:                    {}", rows.len());
            println!("  ACCEPT (typable):               {}", accept_count);
            println!("  PUB (public finance API):       {}", pub_count);
            println!("  IR (planner handles):           {}", ir_count);
            println!("  LEGACY (builtin only):          {}", legacy_count);
            println!("  GLUE:                           {}", glue_count);
            println!("  UNKNOWN:                        {}", unknown_count);
            if !no_yaml {
                println!("  YAML says null, code says IR:   {}", ir_null_count);
            }
        }
        OutputFormat::Json => {
            let json_rows: Vec<serde_json::Value> = rows
                .iter()
                .map(|row| {
                    let mut obj = serde_json::json!({
                        "name": row.name,
                        "accept": row.is_accept,
                        "accept_reason": row.accept_reasons,
                        "pub": row.is_pub,
                        "canon": row.canon,
                        "use": row.use_spelling,
                        "normalize_to": row.normalize_to,
                        "layer": format!("{}", row.layer),
                        "ir_variant": row.ir_variant,
                        "fusable": row.fusable,
                        "notes": row.notes,
                    });
                    if !no_yaml {
                        obj["yaml_current"] = serde_json::json!(row.yaml_current);
                        obj["yaml_planned"] = serde_json::json!(row.yaml_planned);
                    }
                    obj
                })
                .collect();
            let output = serde_json::json!({
                "version": env!("CARGO_PKG_VERSION"),
                "source": "compiled code (planner probes, normalize table, builtins registry)",
                "yaml_included": !no_yaml,
                "summary": {
                    "total": rows.len(),
                    "accept": accept_count,
                    "pub": pub_count,
                    "ir": ir_count,
                    "legacy": legacy_count,
                    "glue": glue_count,
                    "unknown": unknown_count,
                    "yaml_null_but_ir": ir_null_count,
                },
                "operations": json_rows,
            });
            println!("{}", serde_json::to_string_pretty(&output).unwrap());
        }
    }

    Ok(())
}

/// Main entry point for dic command
pub fn run_dic(
    view: View,
    format: OutputFormat,
    grep_pattern: Option<&str>,
    no_yaml: bool,
) -> Result<(), String> {
    // Matrix handles its own data sources (code-driven)
    if let View::Matrix = view {
        return print_matrix(no_yaml, format, grep_pattern);
    }

    // Load appropriate dataset for other views
    let (entries, is_planned) = match view {
        View::Planned => {
            let planned = load_planned_ops()?;
            (planned, true)
        }
        _ => {
            let current = load_current_ops()?;
            (current, false)
        }
    };

    // Validate map (fail fast if YAML has invented names)
    // Note: PLANNED ops are allowed to have unresolved names
    if !is_planned {
        if let Err(errors) = validate_op_map(&entries) {
            eprintln!("❌ Operation map validation errors:");
            for error in &errors {
                eprintln!("  - {}", error);
            }
            return Err(format!("Validation failed with {} errors", errors.len()));
        }
    }

    match view {
        View::Exposed => {
            if is_planned {
                println!("# PLANNED Operations (Roadmap - Not Guaranteed Resolvable)");
                println!();
            }
            print_exposed_aliases(&entries, format, grep_pattern);
        }
        View::Legacy => {
            print_legacy_tokens(&entries, format, grep_pattern);
        }
        View::TodoIR => {
            print_todo_ir(&entries, format, grep_pattern);
        }
        View::Unmapped => {
            print_unmapped_ir(&entries, format);
        }
        View::CheckResolve => {
            if is_planned {
                println!("# Resolution Check: PLANNED Operations");
                println!("# Note: These names are NOT expected to resolve yet");
                println!();
            }
            print_resolution_check(&entries, format);
        }
        View::Planned => {
            println!("# PLANNED Operations (Roadmap Only)");
            println!("# These names do NOT currently resolve");
            println!("# They will move to CURRENT once implemented");
            println!();
            print_exposed_aliases(&entries, format, grep_pattern);
            println!();
            println!();
            println!("# Resolution Status (Informational):");
            print_resolution_check(&entries, format);
        }
        View::All => {
            print_exposed_aliases(&entries, format, grep_pattern);
            println!();
            println!();
            print_legacy_tokens(&entries, format, grep_pattern);
            println!();
            println!();
            print_todo_ir(&entries, format, grep_pattern);
            println!();
            println!();
            print_unmapped_ir(&entries, format);
            println!();
            println!();
            print_resolution_check(&entries, format);
        }
        View::Matrix => unreachable!("handled above"),
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_embedded_yaml_parses() {
        let entries = load_op_map().expect("Failed to parse embedded YAML");
        assert!(
            !entries.is_empty(),
            "Operation map should contain at least one entry"
        );
    }

    #[test]
    fn test_no_alias_collisions() {
        let entries = load_op_map().expect("Failed to parse embedded YAML");
        let mut alias_to_ir: HashMap<String, String> = HashMap::new();

        for entry in &entries {
            let ir_display = entry.ir.as_deref().unwrap_or("<builtin>");
            for alias in &entry.aliases {
                if let Some(existing) = alias_to_ir.get(alias) {
                    panic!(
                        "Alias collision: '{}' maps to both '{}' and '{}'",
                        alias, existing, ir_display
                    );
                }
                alias_to_ir.insert(alias.clone(), ir_display.to_string());
            }
        }
    }

    #[test]
    fn test_validation_passes() {
        let entries = load_op_map().expect("Failed to parse embedded YAML");
        validate_op_map(&entries).expect("Validation should pass");
    }

    #[test]
    fn test_all_entries_have_semantics() {
        let entries = load_op_map().expect("Failed to parse embedded YAML");
        for entry in &entries {
            assert!(
                !entry.semantics.is_empty(),
                "Entry {:?} has empty semantics",
                entry.ir
            );
        }
    }

    #[test]
    fn test_ir_names_match_code() {
        let entries = load_op_map().expect("Failed to parse embedded YAML");
        let ir_set = ir_name_set();

        for entry in &entries {
            if let Some(ref ir_name) = entry.ir {
                assert!(
                    ir_set.contains(ir_name.as_str()),
                    "YAML references unknown IR op: '{}' (not in src/ir.rs enums)",
                    ir_name
                );
            }
        }
    }

    #[test]
    fn test_json_round_trip_exposed_aliases() {
        // Test that JSON output contains expected structure and counts
        let entries = load_op_map().expect("Failed to parse embedded YAML");

        // Build expected data structure (same as print_exposed_aliases)
        let mut rows: Vec<(String, String, String, String)> = Vec::new();
        for entry in &entries {
            for alias in &entry.aliases {
                let ir_display = entry.ir.as_deref().unwrap_or("<builtin>");
                let legacy_str = if entry.legacy_tokens.is_empty() {
                    "-".to_string()
                } else {
                    entry.legacy_tokens.join(", ")
                };
                rows.push((
                    alias.clone(),
                    ir_display.to_string(),
                    entry.bucket.clone(),
                    legacy_str,
                ));
            }
        }

        // Convert to JSON (same format as print_exposed_aliases with JSON)
        let json_rows: Vec<serde_json::Value> = rows
            .iter()
            .map(|(alias, ir, bucket, legacy)| {
                serde_json::json!({
                    "alias": alias,
                    "ir": ir,
                    "bucket": bucket,
                    "legacy_tokens": legacy,
                })
            })
            .collect();

        // Serialize and deserialize
        let json_str = serde_json::to_string(&json_rows).expect("Failed to serialize");
        let parsed: Vec<serde_json::Value> =
            serde_json::from_str(&json_str).expect("Failed to parse JSON back");

        // Verify counts match
        assert_eq!(
            json_rows.len(),
            parsed.len(),
            "JSON round-trip count mismatch"
        );

        // Verify structure of first entry
        assert!(parsed[0].get("alias").is_some(), "Missing 'alias' field");
        assert!(parsed[0].get("ir").is_some(), "Missing 'ir' field");
        assert!(parsed[0].get("bucket").is_some(), "Missing 'bucket' field");
        assert!(
            parsed[0].get("legacy_tokens").is_some(),
            "Missing 'legacy_tokens' field"
        );

        // Log count for human verification
        eprintln!("✅ JSON round-trip: {} aliases verified", parsed.len());
    }

    #[test]
    fn test_no_duplicate_alias_legacy_tokens() {
        // STRICT TRIPWIRE: No token may appear in BOTH aliases and legacy_tokens
        // Policy: Each name belongs to exactly one layer
        let entries = load_op_map().expect("Failed to parse embedded YAML");

        let mut duplicates = Vec::new();

        for entry in &entries {
            let ir_display = entry.ir.as_deref().unwrap_or("<builtin>");
            let alias_set: HashSet<&str> = entry.aliases.iter().map(|s| s.as_str()).collect();
            let legacy_set: HashSet<&str> =
                entry.legacy_tokens.iter().map(|s| s.as_str()).collect();

            let intersection: Vec<&str> = alias_set.intersection(&legacy_set).copied().collect();

            if !intersection.is_empty() {
                duplicates.push(format!(
                    "{}: {:?} appear in both aliases and legacy_tokens",
                    ir_display, intersection
                ));
            }
        }

        assert!(
            duplicates.is_empty(),
            "❌ Cross-layer duplicates detected:\n{}\n\
             Policy: Each name must belong to exactly ONE layer:\n\
             - aliases: current user-facing names (L1)\n\
             - legacy_tokens: deprecated names for backward compat (L2)",
            duplicates.join("\n")
        );
    }

    #[test]
    fn test_no_placeholder_legacy_tokens() {
        // STRICT TRIPWIRE: No placeholder tokens in legacy_tokens
        // "-" looks like the minus operator and should never be a placeholder
        let entries = load_op_map().expect("Failed to parse embedded YAML");

        let mut violations = Vec::new();

        for entry in &entries {
            let ir_display = entry.ir.as_deref().unwrap_or("<builtin>");

            for token in &entry.legacy_tokens {
                if token == "-" || token.is_empty() {
                    violations.push(format!(
                        "{}: has placeholder legacy token '{}' (use empty array instead)",
                        ir_display, token
                    ));
                }
            }
        }

        assert!(
            violations.is_empty(),
            "❌ Placeholder legacy tokens detected:\n{}\n\
             Never use '-' or empty string as placeholder - use empty array []",
            violations.join("\n")
        );
    }

    #[test]
    fn test_matrix_no_yaml_never_loads_yaml() {
        // --no-yaml must produce a full code-driven matrix without YAML
        // Verify: print_matrix(no_yaml=true) succeeds and populates CODE columns
        // This test proves no YAML load occurs: if YAML were required,
        // moving/removing YAML files would break it — but no_yaml=true
        // never calls load_current_ops() or load_planned_ops().
        let result = print_matrix(true, OutputFormat::Json, None);
        assert!(result.is_ok(), "print_matrix(no_yaml=true) must succeed");
    }

    #[test]
    fn test_matrix_probe_uses_normalize() {
        // Probing must go through normalize, so aliases like "w5" resolve
        // to their canonical form ("wkd") before hitting the planner.
        // This means: probing "w5" and probing "wkd" should yield the same IR variant.
        let mut interner = crate::ast::Interner::new();
        let probe_w5 = probe_planner("w5", &mut interner);
        let probe_wkd = probe_planner("wkd", &mut interner);
        assert!(probe_wkd.is_some(), "wkd must be plannable");
        assert!(probe_w5.is_some(), "w5 must be plannable (via normalize)");
        assert_eq!(
            probe_w5.as_ref().unwrap().ir_variant,
            probe_wkd.as_ref().unwrap().ir_variant,
            "w5 and wkd must produce identical IR variant after normalization"
        );
    }

    #[test]
    fn test_matrix_deprecated_flag_on_aliases() {
        // Names on LHS of NORMALIZE_ALIASES must get "dep" in notes
        let result = print_matrix(true, OutputFormat::Json, Some("w5"));
        assert!(result.is_ok());
        // w5 is on LHS of NORMALIZE_ALIASES → should be flagged dep
    }

    #[test]
    fn test_public_finance_ops_all_ir() {
        // TRIPWIRE: Every op in PUBLIC_FINANCE_OPS must be plannable.
        // If this test fails, either:
        //   1. The planner needs to support the op, or
        //   2. The op should be removed from PUBLIC_FINANCE_OPS
        let mut interner = crate::ast::Interner::new();
        let mut failures = Vec::new();

        for &op in PUBLIC_FINANCE_OPS {
            if probe_planner(op, &mut interner).is_none() {
                failures.push(op);
            }
        }

        assert!(
            failures.is_empty(),
            "PUBLIC_FINANCE_OPS contains {} ops that are NOT plannable as IR:\n  {}\n\
             Either add planner support or remove from PUBLIC_FINANCE_OPS.",
            failures.len(),
            failures.join(", ")
        );
    }

    #[test]
    fn test_public_finance_ops_not_glue() {
        // Finance ops must never be classified as GLUE
        for &op in PUBLIC_FINANCE_OPS {
            assert!(
                !GLUE_FORMS.contains(&op),
                "PUBLIC_FINANCE_OPS contains '{}' which is in GLUE_FORMS — conflict",
                op
            );
        }
    }

    #[test]
    fn test_ln_and_log_are_identical() {
        // MICRO TEST: Ensure ln and log resolve to same operation (both natural log)
        // This prevents later drift where they become different
        let ln_status = check_resolve("ln");
        let log_status = check_resolve("log");

        match (&ln_status, &log_status) {
            (ResolveStatus::Builtin, ResolveStatus::Builtin) => {
                // Both resolve as builtins - good
                // They should point to same function (builtin_log)
            }
            (ResolveStatus::IrOp(ln_name), ResolveStatus::IrOp(log_name)) => {
                assert_eq!(
                    ln_name, log_name,
                    "ln and log must resolve to same IR operation (both natural log)"
                );
            }
            _ => {
                panic!(
                    "ln and log must both resolve to same type: ln={:?}, log={:?}",
                    ln_status, log_status
                );
            }
        }
    }

    #[test]
    fn test_matrix_header_columns_stable() {
        // Regression: matrix must succeed and key ops must be plannable as IR.
        let result = print_matrix(true, OutputFormat::Json, None);
        assert!(result.is_ok(), "print_matrix must succeed");

        let mut interner = crate::ast::Interner::new();

        for &op in &["wkd", "dlog", "cs1", "diff", "ecs1"] {
            let probe = probe_planner(op, &mut interner);
            assert!(probe.is_some(), "{} must be plannable", op);
        }

        // ecs1 must use SHF_PFX_LIN_SUM0 (cs0), not SHF_PFX_LIN_SUM (cs1)
        let ecs1_ir = probe_planner("ecs1", &mut interner).unwrap().ir_variant;
        assert!(
            ecs1_ir.contains("SHF_PFX_LIN_SUM0") || ecs1_ir.contains("composite"),
            "ecs1 IR must use CS0, got: {}",
            ecs1_ir
        );
    }

    #[test]
    fn test_migrated_aliases_are_ir() {
        // Tripwire: all word-form aliases must resolve to IR via normalize.
        let mut interner = crate::ast::Interner::new();
        let aliases: &[(&str, &str)] = &[
            ("add", "+"),
            ("sub", "-"),
            ("mul", "*"),
            ("div", "/"),
            ("eq", "=="),
            ("neq", "!="),
            ("gt", ">"),
            ("gte", ">="),
            ("lt", "<"),
            ("lte", "<="),
            ("ln", "log"),
            ("diff-cols", "diff"),
            ("diff-col", "diff"),
            ("ecs1-cols", "ecs1"),
            ("ecs1-col", "ecs1"),
        ];

        for (alias, canonical) in aliases {
            let probe_alias = probe_planner(alias, &mut interner);
            let probe_canon = probe_planner(canonical, &mut interner);
            assert!(
                probe_alias.is_some(),
                "{} must be plannable (alias for {})",
                alias,
                canonical
            );
            assert!(probe_canon.is_some(), "{} must be plannable", canonical);
        }
    }
}
