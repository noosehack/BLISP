// Dictionary module: Read and display canonical operation map
//
// Reads OPS_CANONICAL_MAP.yml embedded at compile time and provides
// multiple views of the operation taxonomy.

use serde::Deserialize;
use std::collections::HashMap;

/// Embedded canonical map (compiled into binary)
const CANONICAL_MAP_YAML: &str = include_str!("../OPS_CANONICAL_MAP.yml");

#[derive(Debug, Clone, Deserialize)]
pub struct OpDef {
    pub canonical: String,
    pub semantics: String,
    pub aliases: Vec<String>,
    pub legacy_tokens: Vec<String>,
    pub ir_node: Option<String>,
    pub ir_ready: bool,
    pub params: Vec<String>,
    pub bucket: String,
    pub notes: String,
    pub semantics_doc: Option<String>,
}

/// Parse the embedded canonical map
pub fn load_canonical_map() -> Result<Vec<OpDef>, String> {
    serde_yaml::from_str(CANONICAL_MAP_YAML)
        .map_err(|e| format!("Failed to parse embedded OPS_CANONICAL_MAP.yml: {}", e))
}

/// Validate the canonical map (check for collisions, format, etc.)
pub fn validate_canonical_map(ops: &[OpDef]) -> Result<(), Vec<String>> {
    let mut errors = Vec::new();

    // Check for alias collisions
    let mut alias_to_canonical: HashMap<String, String> = HashMap::new();
    for op in ops {
        for alias in &op.aliases {
            if let Some(existing) = alias_to_canonical.get(alias) {
                errors.push(format!(
                    "Alias '{}' maps to both '{}' and '{}'",
                    alias, existing, op.canonical
                ));
            } else {
                alias_to_canonical.insert(alias.clone(), op.canonical.clone());
            }
        }
    }

    // Check for legacy token collisions
    let mut token_to_canonical: HashMap<String, String> = HashMap::new();
    for op in ops {
        for token in &op.legacy_tokens {
            if let Some(existing) = token_to_canonical.get(token) {
                errors.push(format!(
                    "Legacy token '{}' maps to both '{}' and '{}'",
                    token, existing, op.canonical
                ));
            } else {
                token_to_canonical.insert(token.clone(), op.canonical.clone());
            }
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

/// Format for output
#[derive(Debug, Clone, Copy)]
pub enum OutputFormat {
    Table,
    Json,
}

/// View selection
#[derive(Debug, Clone, Copy)]
pub enum View {
    Exposed,  // Aliases table
    Legacy,   // Legacy tokens table
    TodoIR,   // IR migration queue
    All,      // All views (default)
}

/// Print exposed aliases table
pub fn print_exposed_aliases(
    ops: &[OpDef],
    format: OutputFormat,
    grep_pattern: Option<&str>,
) {
    let mut rows: Vec<(String, String, bool, String, String)> = Vec::new();

    for op in ops {
        for alias in &op.aliases {
            // Filter by grep pattern if provided
            if let Some(pattern) = grep_pattern {
                if !alias.contains(pattern) && !op.canonical.contains(pattern) {
                    continue;
                }
            }

            let legacy_str = if op.legacy_tokens.is_empty() {
                "-".to_string()
            } else {
                op.legacy_tokens.join(", ")
            };

            rows.push((
                alias.clone(),
                op.canonical.clone(),
                op.ir_ready,
                op.bucket.clone(),
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
                "{:<25} {:<30} {:<10} {:<25} {}",
                "Alias", "Canonical ID", "IR Ready", "Bucket", "Legacy Tokens"
            );
            println!("{}", "-".repeat(120));

            let row_count = rows.len();
            for (alias, canonical, ir_ready, bucket, legacy) in rows {
                let ir_status = if ir_ready { "✅ YES" } else { "❌ NO" };
                println!(
                    "{:<25} {:<30} {:<10} {:<25} {}",
                    alias, canonical, ir_status, bucket, legacy
                );
            }
            println!();
            println!("Total aliases: {}", row_count);
        }
        OutputFormat::Json => {
            let json_rows: Vec<serde_json::Value> = rows
                .iter()
                .map(|(alias, canonical, ir_ready, bucket, legacy)| {
                    serde_json::json!({
                        "alias": alias,
                        "canonical": canonical,
                        "ir_ready": ir_ready,
                        "bucket": bucket,
                        "legacy_tokens": legacy,
                    })
                })
                .collect();
            println!("{}", serde_json::to_string_pretty(&json_rows).unwrap());
        }
    }
}

/// Print legacy tokens table
pub fn print_legacy_tokens(ops: &[OpDef], format: OutputFormat, grep_pattern: Option<&str>) {
    let mut rows: Vec<(String, String, String)> = Vec::new();

    for op in ops {
        for token in &op.legacy_tokens {
            // Filter by grep pattern if provided
            if let Some(pattern) = grep_pattern {
                if !token.contains(pattern) && !op.canonical.contains(pattern) {
                    continue;
                }
            }

            // Find suggested replacement (first alias)
            let suggested = op.aliases.first().cloned().unwrap_or_else(|| "-".to_string());

            rows.push((token.clone(), op.canonical.clone(), suggested));
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
                "Legacy Token", "Canonical ID", "Suggested Replacement"
            );
            println!("{}", "-".repeat(90));

            let row_count = rows.len();
            for (token, canonical, suggested) in rows {
                println!("{:<25} {:<30} {}", token, canonical, suggested);
            }
            println!();
            println!("Total legacy tokens: {}", row_count);
        }
        OutputFormat::Json => {
            let json_rows: Vec<serde_json::Value> = rows
                .iter()
                .map(|(token, canonical, suggested)| {
                    serde_json::json!({
                        "legacy_token": token,
                        "canonical": canonical,
                        "suggested_replacement": suggested,
                    })
                })
                .collect();
            println!("{}", serde_json::to_string_pretty(&json_rows).unwrap());
        }
    }
}

/// Print IR migration queue (ops that need IR migration)
pub fn print_todo_ir(ops: &[OpDef], format: OutputFormat, grep_pattern: Option<&str>) {
    let mut rows: Vec<(String, String, String, Vec<String>)> = Vec::new();

    for op in ops {
        // Filter: only A1/A2 bucket and not IR-ready
        if (op.bucket.starts_with("A1") || op.bucket.starts_with("A2")) && !op.ir_ready {
            // Filter by grep pattern if provided
            if let Some(pattern) = grep_pattern {
                if !op.canonical.contains(pattern)
                    && !op.aliases.iter().any(|a| a.contains(pattern))
                {
                    continue;
                }
            }

            rows.push((
                op.canonical.clone(),
                op.bucket.clone(),
                op.semantics.clone(),
                op.aliases.clone(),
            ));
        }
    }

    // Sort by bucket (A1 first), then canonical
    rows.sort_by(|a, b| {
        let bucket_cmp = a.1.cmp(&b.1);
        if bucket_cmp == std::cmp::Ordering::Equal {
            a.0.cmp(&b.0)
        } else {
            bucket_cmp
        }
    });

    match format {
        OutputFormat::Table => {
            println!("# IR Migration Queue (Not Yet IR-Ready)");
            println!();
            println!(
                "{:<30} {:<25} {:<50} {}",
                "Canonical ID", "Bucket", "Semantics", "Aliases"
            );
            println!("{}", "-".repeat(130));

            for (canonical, bucket, semantics, aliases) in &rows {
                let aliases_str = aliases.join(", ");
                // Truncate semantics if too long
                let semantics_short = if semantics.len() > 47 {
                    format!("{}...", &semantics[..47])
                } else {
                    semantics.clone()
                };

                println!(
                    "{:<30} {:<25} {:<50} {}",
                    canonical, bucket, semantics_short, aliases_str
                );
            }
            println!();
            println!("Total operations needing IR migration: {}", rows.len());

            // Print summary by bucket
            let a1_count = rows.iter().filter(|(_, b, _, _)| b.starts_with("A1")).count();
            let a2_count = rows.iter().filter(|(_, b, _, _)| b.starts_with("A2")).count();
            println!();
            println!("By priority:");
            println!("  A1 (fusion-critical): {} ops", a1_count);
            println!("  A2 (planner-structural): {} ops", a2_count);
        }
        OutputFormat::Json => {
            let json_rows: Vec<serde_json::Value> = rows
                .iter()
                .map(|(canonical, bucket, semantics, aliases)| {
                    serde_json::json!({
                        "canonical": canonical,
                        "bucket": bucket,
                        "semantics": semantics,
                        "aliases": aliases,
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
    let ops = load_canonical_map()?;

    // Validate map
    if let Err(errors) = validate_canonical_map(&ops) {
        eprintln!("❌ Canonical map validation errors:");
        for error in errors {
            eprintln!("  - {}", error);
        }
        return Err("Canonical map validation failed".to_string());
    }

    match view {
        View::Exposed => {
            print_exposed_aliases(&ops, format, grep_pattern);
        }
        View::Legacy => {
            print_legacy_tokens(&ops, format, grep_pattern);
        }
        View::TodoIR => {
            print_todo_ir(&ops, format, grep_pattern);
        }
        View::All => {
            print_exposed_aliases(&ops, format, grep_pattern);
            println!();
            println!();
            print_legacy_tokens(&ops, format, grep_pattern);
            println!();
            println!();
            print_todo_ir(&ops, format, grep_pattern);
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_embedded_yaml_parses() {
        let ops = load_canonical_map().expect("Failed to parse embedded YAML");
        assert!(
            !ops.is_empty(),
            "Canonical map should contain at least one operation"
        );
    }

    #[test]
    fn test_no_alias_collisions() {
        let ops = load_canonical_map().expect("Failed to parse embedded YAML");
        let mut alias_to_canonical: HashMap<String, String> = HashMap::new();

        for op in &ops {
            for alias in &op.aliases {
                if let Some(existing) = alias_to_canonical.get(alias) {
                    panic!(
                        "Alias collision: '{}' maps to both '{}' and '{}'",
                        alias, existing, op.canonical
                    );
                }
                alias_to_canonical.insert(alias.clone(), op.canonical.clone());
            }
        }
    }

    #[test]
    fn test_validation_passes() {
        let ops = load_canonical_map().expect("Failed to parse embedded YAML");
        validate_canonical_map(&ops).expect("Validation should pass");
    }

    #[test]
    fn test_all_canonicals_have_semantics() {
        let ops = load_canonical_map().expect("Failed to parse embedded YAML");
        for op in &ops {
            assert!(
                !op.semantics.is_empty(),
                "Canonical '{}' has empty semantics",
                op.canonical
            );
        }
    }

    #[test]
    fn test_ir_ready_ops_have_ir_node() {
        let ops = load_canonical_map().expect("Failed to parse embedded YAML");
        for op in &ops {
            if op.ir_ready {
                assert!(
                    op.ir_node.is_some(),
                    "Canonical '{}' is IR-ready but has no ir_node",
                    op.canonical
                );
            }
        }
    }
}
