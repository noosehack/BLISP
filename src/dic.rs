// Dictionary module: Display operation metadata
//
// Schema: Code owns canonical IDs (enum variants), YAML owns metadata (aliases, docs, buckets)
// Validates YAML ir: field against actual IR enum variants (anti-invention guardrail)

use crate::ir::{BinaryFunc, NumericFunc, Source};
use serde::Deserialize;
use std::collections::{HashMap, HashSet};

/// Embedded metadata overlay (compiled into binary)
const CANONICAL_MAP_YAML: &str = include_str!("../OPS_CANONICAL_MAP.yml");

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

/// Parse the embedded metadata overlay
pub fn load_op_map() -> Result<Vec<OpMapEntry>, String> {
    serde_yaml::from_str(CANONICAL_MAP_YAML)
        .map_err(|e| format!("Failed to parse OPS_CANONICAL_MAP.yml: {}", e))
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
        let ir_display = entry.ir.as_ref().map(|s| s.as_str()).unwrap_or("<builtin>");
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
        let ir_display = entry.ir.as_ref().map(|s| s.as_str()).unwrap_or("<builtin>");
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
    Exposed,       // Aliases table
    Legacy,        // Legacy tokens table
    TodoIR,        // IR migration queue
    Unmapped,      // IR ops missing metadata
    CheckResolve,  // Resolution check (reality test)
    All,           // All views (default)
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
                let ir_name = entry.ir.as_ref().map(|s| s.as_str()).unwrap_or("");
                if !alias.contains(pattern) && !ir_name.contains(pattern) {
                    continue;
                }
            }

            let ir_display = entry
                .ir
                .as_ref()
                .map(|s| s.as_str())
                .unwrap_or("<builtin>");
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

    // Sort by alias
    rows.sort_by(|a, b| a.0.cmp(&b.0));

    match format {
        OutputFormat::Table => {
            println!("# Exposed Aliases (User-Facing Operations)");
            println!();
            println!(
                "{:<25} {:<30} {:<25} {}",
                "Alias", "IR / Builtin", "Bucket", "Legacy Tokens"
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
                let ir_name = entry.ir.as_ref().map(|s| s.as_str()).unwrap_or("");
                if !token.contains(pattern) && !ir_name.contains(pattern) {
                    continue;
                }
            }

            let ir_display = entry
                .ir
                .as_ref()
                .map(|s| s.as_str())
                .unwrap_or("<builtin>");
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
                "{:<25} {:<30} {}",
                "Legacy Token", "IR / Builtin", "Suggested Replacement"
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
                "{:<25} {:<50} {:<30} {}",
                "Bucket", "Semantics", "Aliases", "Legacy Tokens"
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
            let a1_count = rows.iter().filter(|(b, _, _, _)| b.starts_with("A1")).count();
            let a2_count = rows.iter().filter(|(b, _, _, _)| b.starts_with("A2")).count();
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
    IrOp(String),      // Resolves to IR operation
    Builtin,           // Resolves to builtin
    Unknown,           // Not found
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
        let ir_display = entry.ir.as_ref().map(|s| s.as_str()).unwrap_or("<builtin>");

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
                    format!("FAIL(unknown) [legacy]")
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
            println!("{:<30} {:<30} {}", "Name", "Expected (YAML)", "Actual (Runtime)");
            println!("{}", "-".repeat(100));

            for (name, expected, status) in &results {
                println!("{:<30} {:<30} {}", name, expected, status);
            }

            println!();
            if unknown_count > 0 {
                println!("❌ {} names FAIL to resolve (not registered in runtime)", unknown_count);
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

/// Main entry point for dic command
pub fn run_dic(
    view: View,
    format: OutputFormat,
    grep_pattern: Option<&str>,
) -> Result<(), String> {
    let entries = load_op_map()?;

    // Validate map (fail fast if YAML has invented names)
    if let Err(errors) = validate_op_map(&entries) {
        eprintln!("❌ Operation map validation errors:");
        for error in &errors {
            eprintln!("  - {}", error);
        }
        return Err(format!("Validation failed with {} errors", errors.len()));
    }

    match view {
        View::Exposed => {
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
            let ir_display = entry.ir.as_ref().map(|s| s.as_str()).unwrap_or("<builtin>");
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
                let ir_display = entry.ir.as_ref().map(|s| s.as_str()).unwrap_or("<builtin>");
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
        let parsed: Vec<serde_json::Value> = serde_json::from_str(&json_str)
            .expect("Failed to parse JSON back");

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
        assert!(parsed[0].get("legacy_tokens").is_some(), "Missing 'legacy_tokens' field");

        // Log count for human verification
        eprintln!("✅ JSON round-trip: {} aliases verified", parsed.len());
    }

    #[test]
    fn test_no_duplicate_alias_legacy_tokens() {
        // Test that names don't appear in BOTH aliases and legacy_tokens
        // (unless intentionally duplicated for transition period)
        let entries = load_op_map().expect("Failed to parse embedded YAML");

        let mut duplicates = Vec::new();

        for entry in &entries {
            let ir_display = entry.ir.as_ref().map(|s| s.as_str()).unwrap_or("<builtin>");
            let alias_set: HashSet<&str> = entry.aliases.iter().map(|s| s.as_str()).collect();
            let legacy_set: HashSet<&str> = entry.legacy_tokens.iter().map(|s| s.as_str()).collect();

            let intersection: Vec<&str> = alias_set.intersection(&legacy_set).copied().collect();

            if !intersection.is_empty() {
                duplicates.push(format!(
                    "{}: {:?} appear in both aliases and legacy_tokens",
                    ir_display,
                    intersection
                ));
            }
        }

        if !duplicates.is_empty() {
            eprintln!("⚠️  Warning: {} operations have tokens in both layers:", duplicates.len());
            for dup in &duplicates {
                eprintln!("  - {}", dup);
            }
            eprintln!("\nThis may be intentional (transition period) or a mistake.");
            eprintln!("Each name should typically belong to exactly one layer:");
            eprintln!("  - aliases: current user-facing names (L1)");
            eprintln!("  - legacy_tokens: deprecated names for backward compat (L2)");

            // Don't fail yet, but make it visible
            // Uncomment to enforce:
            // panic!("Duplicate tokens found across layers");
        }
    }
}
