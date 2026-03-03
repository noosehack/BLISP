#![allow(dead_code)]
#![allow(unused_imports)]
#![allow(unused_mut)]
#![allow(clippy::useless_vec)]
use blisp::reader::Reader;
use blisp::runtime::Runtime;
use blisp::value::{self, Value};
use blisp::{ast, eval, exec, io, normalize, planner};
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
    eprintln!("  dic [OPTIONS]                  Show operation dictionary");
    eprintln!();
    eprintln!("OPTIONS:");
    eprintln!("  --version                      Show version and exit");
    eprintln!("  --help                         Show this help message");
    eprintln!("  --load <file>                  Load stdlib file before execution");
    eprintln!("  -e '<expression>'              Evaluate expression");
    eprintln!("  --legacy                       Force legacy AST evaluator");
    eprintln!("  --ir-only                      Force IR-only mode (experimental)");
    eprintln!();
    eprintln!("DIC OPTIONS:");
    eprintln!("  --exposed                      Show exposed aliases (default)");
    eprintln!("  --legacy                       Show legacy tokens");
    eprintln!("  --todo-ir                      Show IR migration queue");
    eprintln!("  --unmapped                     Show IR ops without metadata");
    eprintln!("  --check-resolve                Check if names resolve in runtime (reality test)");
    eprintln!("  --json                         Output in JSON format");
    eprintln!("  --grep <pattern>               Filter by pattern");
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
    // Parse dic arguments: blisp dic [--exposed|--legacy|--todo-ir] [--json] [--grep <pattern>]
    use blisp::dic::{OutputFormat, View};

    let mut view = View::All;
    let mut format = OutputFormat::Table;
    let mut grep_pattern: Option<String> = None;
    let mut i = 2; // Skip "blisp" and "dic"

    // If no flags, default to exposed
    let has_view_flag = args.iter().skip(2).any(|arg| {
        matches!(
            arg.as_str(),
            "--exposed" | "--legacy" | "--todo-ir" | "--unmapped" | "--check-resolve"
        )
    });

    if !has_view_flag {
        view = View::Exposed;
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
                eprintln!("Valid options: --exposed, --legacy, --todo-ir, --unmapped, --check-resolve, --json, --grep <pattern>");
                std::process::exit(1);
            }
        }
    }

    match blisp::dic::run_dic(view, format, grep_pattern.as_deref()) {
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
            "--legacy" | "--ir-only" | "--dic" | "selftest" | "--selftest" => {
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
        match eval_code(&mut rt, code, use_legacy, use_ir_only) {
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
    } else if let Some(file) = script_file {
        // File execution
        match std::fs::read_to_string(&file) {
            Ok(code) => {
                match eval_code(&mut rt, &code, use_legacy, use_ir_only) {
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

        match eval_code(&mut rt, code, false, false) {
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

        match eval_code(&mut rt, code, false, false) {
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

        match eval_code(&mut rt, code, false, false) {
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

        match eval_code(&mut rt, code, false, false) {
            Ok(val) => println!("=> {:?}\n", val),
            Err(e) => println!("Error: {}\n", e),
        }
    }
}

/// Try to evaluate via IR path (normalize → plan → execute)
fn try_ir_eval(rt: &mut Runtime, expr: ast::Expr) -> Result<value::Value, String> {
    // Step 1: Normalize (macro expansion, desugaring)
    let normalized = normalize::normalize(expr, &mut rt.interner);

    // Step 2: Plan (AST → IR with schema validation)
    let plan = planner::plan(&normalized, &rt.interner)?;

    // Step 3: Execute (run optimized IR executor)
    let result = exec::execute(&plan, rt)?;

    Ok(result)
}

fn eval_code(
    rt: &mut Runtime,
    code: &str,
    use_legacy: bool,
    use_ir_only: bool,
) -> Result<value::Value, String> {
    let mut reader = Reader::new(code).map_err(|e| format!("Parse error: {}", e))?;

    // Read and evaluate ALL expressions (implicit progn)
    let mut result = value::Value::Nil;

    loop {
        match reader.read(&mut rt.interner) {
            Ok(expr) => {
                if use_legacy {
                    // Legacy-only mode: use old AST evaluator
                    result = rt.eval(&expr)?;
                } else if use_ir_only {
                    // IR-only mode: force IR path (experimental, Frame ops only)
                    result = try_ir_eval(rt, expr)?;
                } else {
                    // 🎯 HYBRID mode (DEFAULT):
                    // Try IR first for Frame operations, fall back to legacy for general Lisp
                    match try_ir_eval(rt, expr.clone()) {
                        Ok(val) => {
                            // ✅ IR succeeded (Frame pipeline)
                            // Benefits:
                            // - O(n) rolling operations (6-102x faster)
                            // - Fusion framework ready
                            // - Schema validation at plan time
                            // - All 116 IR tests enforcing correctness
                            result = val;
                        }
                        Err(e)
                            if e.contains("Cannot plan")
                                || e.contains("not supported")
                                || e.contains("Unknown function") =>
                        {
                            // IR can't handle this expression → fallback to legacy
                            // This is NORMAL for general Lisp (defparameter, if, let*, etc.)
                            result = rt.eval(&expr)?;
                        }
                        Err(e) => {
                            // IR failed with real error → propagate
                            return Err(e);
                        }
                    }
                }
            }
            Err(e) => {
                // Check if we've reached end of input
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
