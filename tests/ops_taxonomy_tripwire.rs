// Tripwire Test: Operation Taxonomy Invariants
//
// Purpose: Enforce canonical operation map consistency
// Fails if:
// - An alias points to multiple canonicals
// - A canonical has no semantics doc anchor
// - A legacy token exists without mapping
// - Naming conventions are violated
//
// This test reads OPS_CANONICAL_MAP.yml and validates all invariants.

use std::collections::{HashMap, HashSet};
use std::fs;

#[derive(Debug, serde::Deserialize)]
struct OpDef {
    canonical: String,
    semantics: String,
    aliases: Vec<String>,
    legacy_tokens: Vec<String>,
    ir_node: Option<String>,
    ir_ready: bool,
    params: Vec<String>,
    bucket: String,
    notes: String,
    semantics_doc: Option<String>,
}

#[test]
fn tripwire_no_alias_overloading() {
    // Rule: Each alias must map to exactly one canonical ID
    let ops = load_canonical_map();

    let mut alias_to_canonical: HashMap<String, String> = HashMap::new();
    let mut violations = Vec::new();

    for op in &ops {
        for alias in &op.aliases {
            if let Some(existing_canonical) = alias_to_canonical.get(alias) {
                violations.push(format!(
                    "Alias '{}' maps to both '{}' and '{}'",
                    alias, existing_canonical, op.canonical
                ));
            } else {
                alias_to_canonical.insert(alias.clone(), op.canonical.clone());
            }
        }
    }

    if !violations.is_empty() {
        panic!(
            "❌ Alias overloading detected:\n{}",
            violations.join("\n")
        );
    }
}

#[test]
fn tripwire_no_legacy_token_overloading() {
    // Rule: Each legacy token must map to exactly one canonical ID
    let ops = load_canonical_map();

    let mut token_to_canonical: HashMap<String, String> = HashMap::new();
    let mut violations = Vec::new();

    for op in &ops {
        for token in &op.legacy_tokens {
            if let Some(existing_canonical) = token_to_canonical.get(token) {
                violations.push(format!(
                    "Legacy token '{}' maps to both '{}' and '{}'",
                    token, existing_canonical, op.canonical
                ));
            } else {
                token_to_canonical.insert(token.clone(), op.canonical.clone());
            }
        }
    }

    if !violations.is_empty() {
        panic!(
            "❌ Legacy token overloading detected:\n{}",
            violations.join("\n")
        );
    }
}

#[test]
fn tripwire_all_canonicals_documented() {
    // Rule: Every canonical must have a semantics_doc anchor
    // (except utility ops which are not fuseable)
    let ops = load_canonical_map();

    let mut violations = Vec::new();

    for op in &ops {
        if op.semantics_doc.is_none() && !op.bucket.starts_with("A3_edge") {
            violations.push(format!(
                "Canonical '{}' has no semantics_doc anchor (bucket: {})",
                op.canonical, op.bucket
            ));
        }
    }

    if !violations.is_empty() {
        panic!(
            "❌ Missing semantics documentation:\n{}",
            violations.join("\n")
        );
    }
}

#[test]
fn tripwire_canonical_id_format() {
    // Rule: Canonical IDs must follow ISO-like naming convention
    // Format: PREFIX_CATEGORY_TYPE_NAME
    // Prefixes: MSK, SHF, CUM, WIN, BIN, UNY, CMP, AGG, TBL, IO, FIN, UTL
    let ops = load_canonical_map();

    let valid_prefixes = vec![
        "MSK", "SHF", "CUM", "WIN", "BIN", "UNY", "CMP",
        "AGG", "TBL", "IO", "FIN", "UTL"
    ];

    let mut violations = Vec::new();

    for op in &ops {
        let parts: Vec<&str> = op.canonical.split('_').collect();
        if parts.is_empty() {
            violations.push(format!(
                "Canonical '{}' has no underscore separators",
                op.canonical
            ));
            continue;
        }

        if !valid_prefixes.contains(&parts[0]) {
            violations.push(format!(
                "Canonical '{}' has invalid prefix '{}' (expected one of: {})",
                op.canonical,
                parts[0],
                valid_prefixes.join(", ")
            ));
        }

        // Must be ALL_CAPS_WITH_UNDERSCORES
        if op.canonical != op.canonical.to_uppercase() {
            violations.push(format!(
                "Canonical '{}' is not all uppercase",
                op.canonical
            ));
        }
    }

    if !violations.is_empty() {
        panic!(
            "❌ Canonical ID format violations:\n{}",
            violations.join("\n")
        );
    }
}

#[test]
fn tripwire_alias_naming_conventions() {
    // Rule: Aliases must be dash-separated, lowercase, no underscores
    let ops = load_canonical_map();

    let mut violations = Vec::new();

    for op in &ops {
        for alias in &op.aliases {
            // Skip operators like +, -, *, /, <, >, etc.
            if alias.chars().all(|c| !c.is_alphanumeric()) {
                continue;
            }

            // Must be lowercase (except first char if it's an operator)
            if alias != &alias.to_lowercase() {
                violations.push(format!(
                    "Alias '{}' (for {}) contains uppercase letters",
                    alias, op.canonical
                ));
            }

            // No underscores
            if alias.contains('_') {
                violations.push(format!(
                    "Alias '{}' (for {}) contains underscores (use dashes instead)",
                    alias, op.canonical
                ));
            }
        }
    }

    if !violations.is_empty() {
        panic!(
            "❌ Alias naming convention violations:\n{}",
            violations.join("\n")
        );
    }
}

#[test]
fn tripwire_bucket_validity() {
    // Rule: Every operation must have a valid bucket
    let ops = load_canonical_map();

    let valid_buckets = vec![
        "A1_fusion_critical",
        "A2_planner_structural",
        "A3_edge_io",
    ];

    let mut violations = Vec::new();

    for op in &ops {
        if !valid_buckets.contains(&op.bucket.as_str()) {
            violations.push(format!(
                "Operation '{}' has invalid bucket '{}' (expected one of: {})",
                op.canonical,
                op.bucket,
                valid_buckets.join(", ")
            ));
        }
    }

    if !violations.is_empty() {
        panic!(
            "❌ Invalid buckets:\n{}",
            violations.join("\n")
        );
    }
}

#[test]
fn tripwire_ir_ready_consistency() {
    // Rule: If ir_ready=true, must have ir_node
    let ops = load_canonical_map();

    let mut violations = Vec::new();

    for op in &ops {
        if op.ir_ready && op.ir_node.is_none() {
            violations.push(format!(
                "Operation '{}' is marked ir_ready=true but has no ir_node",
                op.canonical
            ));
        }
    }

    if !violations.is_empty() {
        panic!(
            "❌ IR readiness inconsistencies:\n{}",
            violations.join("\n")
        );
    }
}

#[test]
fn tripwire_no_orphaned_legacy_tokens() {
    // Rule: Every legacy token in the codebase must be mapped
    // This is a "soft" tripwire - we'll collect unmapped tokens but not fail yet
    let ops = load_canonical_map();

    // Collect all mapped legacy tokens
    let mut mapped_tokens: HashSet<String> = HashSet::new();
    for op in &ops {
        for token in &op.legacy_tokens {
            mapped_tokens.insert(token.clone());
        }
        for alias in &op.aliases {
            mapped_tokens.insert(alias.clone());
        }
    }

    // Extract tokens from actual codebase
    let builtin_tokens = extract_builtin_tokens();
    let planner_tokens = extract_planner_tokens();

    let mut unmapped_builtins = Vec::new();
    let mut unmapped_planner = Vec::new();

    for token in &builtin_tokens {
        if !mapped_tokens.contains(token) {
            unmapped_builtins.push(token.clone());
        }
    }

    for token in &planner_tokens {
        if !mapped_tokens.contains(token) {
            unmapped_planner.push(token.clone());
        }
    }

    if !unmapped_builtins.is_empty() || !unmapped_planner.is_empty() {
        eprintln!("⚠️  Warning: Unmapped tokens found in codebase:");
        if !unmapped_builtins.is_empty() {
            eprintln!("  Builtins: {}", unmapped_builtins.join(", "));
        }
        if !unmapped_planner.is_empty() {
            eprintln!("  Planner: {}", unmapped_planner.join(", "));
        }
        eprintln!("  → Add these to OPS_CANONICAL_MAP.yml");

        // Don't fail yet - this is informational for now
        // Uncomment to enforce:
        // panic!("Unmapped tokens detected");
    }
}

#[test]
fn tripwire_migration_priority_order() {
    // Rule: A1 ops should be moved to IR before A2 ops
    // This test tracks which A1 ops are still not IR-ready
    let ops = load_canonical_map();

    let mut a1_not_ready = Vec::new();
    let mut a2_ready = Vec::new();

    for op in &ops {
        if op.bucket == "A1_fusion_critical" && !op.ir_ready {
            a1_not_ready.push(op.canonical.clone());
        }
        if op.bucket == "A2_planner_structural" && op.ir_ready {
            a2_ready.push(op.canonical.clone());
        }
    }

    if !a1_not_ready.is_empty() {
        eprintln!("ℹ️  A1 (fusion-critical) ops not yet IR-ready:");
        for op in &a1_not_ready {
            eprintln!("  - {}", op);
        }
        eprintln!("  → These should be prioritized for IR migration");
    }

    if !a2_ready.is_empty() {
        eprintln!("ℹ️  A2 (structural) ops already IR-ready:");
        for op in &a2_ready {
            eprintln!("  - {}", op);
        }
    }

    // Don't fail - this is tracking info
    // Could enforce later: assert!(a1_not_ready.is_empty());
}

// =============================================================================
// Helper Functions
// =============================================================================

fn load_canonical_map() -> Vec<OpDef> {
    let yaml_path = concat!(env!("CARGO_MANIFEST_DIR"), "/OPS_CANONICAL_MAP.yml");
    let contents = fs::read_to_string(yaml_path)
        .unwrap_or_else(|e| panic!("Failed to read OPS_CANONICAL_MAP.yml: {}", e));

    serde_yaml::from_str(&contents)
        .unwrap_or_else(|e| panic!("Failed to parse OPS_CANONICAL_MAP.yml: {}", e))
}

fn extract_builtin_tokens() -> HashSet<String> {
    // TODO: Parse src/builtins.rs register_builtins function
    // For now, return empty set (implement when needed)
    HashSet::new()
}

fn extract_planner_tokens() -> HashSet<String> {
    // TODO: Parse src/planner.rs match statements
    // For now, return empty set (implement when needed)
    HashSet::new()
}
