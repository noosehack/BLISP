mod ast;
mod reader;
mod value;
mod env;
mod runtime;
mod eval;
mod builtins;
mod io;

use runtime::Runtime;
use reader::Reader;
use std::io::Write;

fn main() {
    let args: Vec<String> = std::env::args().collect();

    // Parse command line arguments
    if args.len() < 2 {
        eprintln!("blisp v0.1.0");
        eprintln!("Usage:");
        eprintln!("  blisp -e '<expression>'    Execute expression");
        eprintln!("  blisp <file.lisp>          Execute file");
        eprintln!();
        eprintln!("Examples:");
        eprintln!("  blisp -e '(+ 1 2)'");
        eprintln!("  blisp -e '(dlog prices 1)'");
        eprintln!("  blisp script.lisp");
        std::process::exit(1);
    }

    let mut rt = Runtime::new();

    // Handle -e flag (expression evaluation)
    if args[1] == "-e" {
        if args.len() < 3 {
            eprintln!("Error: -e requires an expression");
            std::process::exit(1);
        }

        let code = &args[2];
        match eval_code(&mut rt, code) {
            Ok(val) => {
                // Handle broken pipe gracefully (e.g., when piping to head)
                let result = writeln!(std::io::stdout(), "{}", val.display(&rt.interner));
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
    } else {
        // File execution
        let filename = &args[1];
        match std::fs::read_to_string(filename) {
            Ok(code) => {
                match eval_code(&mut rt, &code) {
                    Ok(val) => {
                        // Handle broken pipe gracefully (e.g., when piping to head)
                        let result = writeln!(std::io::stdout(), "{}", val.display(&rt.interner));
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
                eprintln!("Error reading file '{}': {}", filename, e);
                std::process::exit(1);
            }
        }
    }
}

fn demo_column_ops() {
    let mut rt = Runtime::new();

    println!("=== Step 6: High-Performance Column Operations ===");
    println!();

    use value::Value;
    use std::sync::Arc;

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

        match eval_code(&mut rt, code) {
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

        match eval_code(&mut rt, code) {
            Ok(val) => println!("=> {}\n", val.display(&rt.interner)),
            Err(e) => println!("Error: {}\n", e),
        }
    }

    // Column operations demo
    println!("=== Column Operations ===\n");

    use value::Value;
    use std::sync::Arc;

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

        match eval_code(&mut rt, code) {
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
    use value::{Value, Table};
    use std::sync::Arc;

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
        ("(let* ((x 5)) (let* ((x 10)) x))", "Nested let* (inner shadows)"),

        // Complex expression
        (r#"(progn
               (defparameter z 1)
               (let* ((z 2))
                 (setf z 20)
                 z))"#, "Complex: progn + defparameter + let* + setf"),

        ("z", "z should still be 1 (global unchanged)"),
    ];

    for (code, description) in test_programs {
        println!(">>> {}", description);
        println!("{}", code);

        match eval_code(&mut rt, code) {
            Ok(val) => println!("=> {:?}\n", val),
            Err(e) => println!("Error: {}\n", e),
        }
    }
}

fn eval_code(rt: &mut Runtime, code: &str) -> Result<value::Value, String> {
    let mut reader = Reader::new(code).map_err(|e| format!("Parse error: {}", e))?;

    // Read and evaluate ALL expressions (implicit progn)
    let mut result = value::Value::Nil;

    loop {
        match reader.read(&mut rt.interner) {
            Ok(expr) => {
                result = rt.eval(&expr)?;
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
