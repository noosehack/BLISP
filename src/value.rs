//! Runtime values for blisp
//!
//! Step 4: Full implementation with Col and Table types.

use crate::ast::SymbolId;
use std::sync::Arc;

/// Convert days since epoch to YYYY-MM-DD (Howard Hinnant algorithm)
fn format_date(days: i32) -> String {
    if days == blawktrust::NULL_DATE {
        return "NA".to_string();
    }

    // Howard Hinnant's inverse algorithm
    let z = days + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = (z - era * 146097) as u32;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe as i32 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = if m <= 2 { y + 1 } else { y };

    format!("{:04}-{:02}-{:02}", year, m, d)
}

/// Format nanoseconds as YYYY-MM-DD HH:MM:SS[.fffffffff]
fn format_timestamp(nanos: i64) -> String {
    if nanos == blawktrust::NULL_TIMESTAMP {
        return "NA".to_string();
    }

    let total_seconds = nanos / 1_000_000_000;
    let frac_nanos = nanos % 1_000_000_000;

    let days = total_seconds / 86400;
    let remaining_secs = total_seconds % 86400;

    let hour = remaining_secs / 3600;
    let minute = (remaining_secs % 3600) / 60;
    let second = remaining_secs % 60;

    let date_str = format_date(days as i32);

    if frac_nanos == 0 {
        format!("{} {:02}:{:02}:{:02}", date_str, hour, minute, second)
    } else {
        let frac_str = format!("{:09}", frac_nanos);
        let trimmed = frac_str.trim_end_matches('0');
        format!("{} {:02}:{:02}:{:02}.{}", date_str, hour, minute, second, trimmed)
    }
}

/// Display a column with proper formatting
fn display_column(col: &blawktrust::Column) -> String {
    const MAX_SHOW: usize = 12; // Show first 10, last 2 if large

    match col {
        blawktrust::Column::F64(data) => {
            let len = data.len();
            if len == 0 {
                return "F64[]".to_string();
            }

            let mut parts = vec!["F64[".to_string()];

            if len <= MAX_SHOW {
                for (i, &val) in data.iter().enumerate() {
                    if i > 0 { parts.push(", ".to_string()); }
                    if val.is_nan() {
                        parts.push("NA".to_string());
                    } else {
                        parts.push(format!("{}", val));
                    }
                }
            } else {
                // Show first 10
                for i in 0..10 {
                    if i > 0 { parts.push(", ".to_string()); }
                    let val = data[i];
                    if val.is_nan() {
                        parts.push("NA".to_string());
                    } else {
                        parts.push(format!("{}", val));
                    }
                }
                parts.push(", ...".to_string());
                // Show last 2
                for i in (len - 2)..len {
                    parts.push(", ".to_string());
                    let val = data[i];
                    if val.is_nan() {
                        parts.push("NA".to_string());
                    } else {
                        parts.push(format!("{}", val));
                    }
                }
            }

            parts.push(format!("] (n={})", len));
            parts.concat()
        }
        blawktrust::Column::Date(data) => {
            let len = data.len();
            if len == 0 {
                return "Date[]".to_string();
            }

            let mut parts = vec!["Date[".to_string()];

            if len <= MAX_SHOW {
                for (i, &val) in data.iter().enumerate() {
                    if i > 0 { parts.push(", ".to_string()); }
                    parts.push(format_date(val));
                }
            } else {
                // Show first 10
                for i in 0..10 {
                    if i > 0 { parts.push(", ".to_string()); }
                    parts.push(format_date(data[i]));
                }
                parts.push(", ...".to_string());
                // Show last 2
                for i in (len - 2)..len {
                    parts.push(", ".to_string());
                    parts.push(format_date(data[i]));
                }
            }

            parts.push(format!("] (n={})", len));
            parts.concat()
        }
        blawktrust::Column::Timestamp(data) => {
            let len = data.len();
            if len == 0 {
                return "Timestamp[]".to_string();
            }

            let mut parts = vec!["Timestamp[".to_string()];

            if len <= MAX_SHOW {
                for (i, &val) in data.iter().enumerate() {
                    if i > 0 { parts.push(", ".to_string()); }
                    parts.push(format_timestamp(val));
                }
            } else {
                // Show first 10
                for i in 0..10 {
                    if i > 0 { parts.push(", ".to_string()); }
                    parts.push(format_timestamp(data[i]));
                }
                parts.push(", ...".to_string());
                // Show last 2
                for i in (len - 2)..len {
                    parts.push(", ".to_string());
                    parts.push(format_timestamp(data[i]));
                }
            }

            parts.push(format!("] (n={})", len));
            parts.concat()
        }
    }
}

/// Write table as CSV (semicolon-separated) with streaming output
///
/// Writes incrementally to avoid building giant strings in memory.
/// Returns Err on broken pipe or other I/O errors.
pub fn write_table_to<W: std::io::Write>(
    writer: &mut W,
    table: &Table,
    interner: &crate::ast::Interner,
    max_rows: Option<usize>,
) -> std::io::Result<()> {
    if table.columns.is_empty() {
        return Ok(());
    }

    let n_rows = table.row_count;
    let display_rows = max_rows.map(|m| n_rows.min(m)).unwrap_or(n_rows);

    // Column headers
    let col_names: Vec<String> = table.columns.iter()
        .map(|(sym, _)| interner.resolve(*sym).to_string())
        .collect();

    writeln!(writer, "{}", col_names.join(";"))?;

    // Data rows (streaming)
    for row_idx in 0..display_rows {
        for (col_idx, (_, col)) in table.columns.iter().enumerate() {
            if col_idx > 0 {
                write!(writer, ";")?;
            }

            match col {
                blawktrust::Column::F64(data) => {
                    if row_idx < data.len() {
                        let v = data[row_idx];
                        if v.is_nan() {
                            write!(writer, "NA")?;
                        } else {
                            write!(writer, "{}", v)?;
                        }
                    } else {
                        write!(writer, "?")?;
                    }
                }
                blawktrust::Column::Date(data) => {
                    if row_idx < data.len() {
                        write!(writer, "{}", format_date(data[row_idx]))?;
                    } else {
                        write!(writer, "?")?;
                    }
                }
                blawktrust::Column::Timestamp(data) => {
                    if row_idx < data.len() {
                        write!(writer, "{}", format_timestamp(data[row_idx]))?;
                    } else {
                        write!(writer, "?")?;
                    }
                }
            }
        }
        writeln!(writer)?;
    }

    // Show summary if truncated
    if display_rows < n_rows {
        writeln!(writer, "... ({} more rows, {} total)", n_rows - display_rows, n_rows)?;
    }

    Ok(())
}

/// Table: columnar data structure
#[derive(Debug, Clone)]
pub struct Table {
    pub columns: Vec<(SymbolId, blawktrust::Column)>,
    pub row_count: usize,
}

/// Runtime value
#[derive(Debug, Clone)]
pub enum Value {
    Nil,
    Bool(bool),
    Int(i64),
    Float(f64),
    Str(Arc<str>),
    Sym(SymbolId),
    List(Vec<Value>),
    Col(Arc<blawktrust::Column>),
    Table(Arc<Table>),
    TableView(Arc<blawktrust::TableView>),
}

// Manual PartialEq because Column doesn't implement it
impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Value::Nil, Value::Nil) => true,
            (Value::Bool(a), Value::Bool(b)) => a == b,
            (Value::Int(a), Value::Int(b)) => a == b,
            (Value::Float(a), Value::Float(b)) => a == b,
            (Value::Str(a), Value::Str(b)) => a == b,
            (Value::Sym(a), Value::Sym(b)) => a == b,
            (Value::List(a), Value::List(b)) => a == b,
            // Columns compare by pointer for now
            (Value::Col(a), Value::Col(b)) => Arc::ptr_eq(a, b),
            (Value::Table(a), Value::Table(b)) => Arc::ptr_eq(a, b),
            (Value::TableView(a), Value::TableView(b)) => Arc::ptr_eq(a, b),
            _ => false,
        }
    }
}

impl Value {
    /// Get the type name of this value
    pub fn type_name(&self) -> &str {
        match self {
            Value::Nil => "nil",
            Value::Bool(_) => "bool",
            Value::Int(_) => "int",
            Value::Float(_) => "float",
            Value::Str(_) => "str",
            Value::Sym(_) => "sym",
            Value::List(_) => "list",
            Value::Col(_) => "col",
            Value::Table(_) => "table",
            Value::TableView(_) => "tableview",
        }
    }

    /// Check if value is truthy (for if conditionals)
    pub fn is_truthy(&self) -> bool {
        !matches!(self, Value::Nil | Value::Bool(false))
    }

    /// Extract as int
    pub fn as_int(&self) -> Result<i64, String> {
        match self {
            Value::Int(n) => Ok(*n),
            _ => Err(format!("Expected int, got {}", self.type_name())),
        }
    }

    /// Extract as float (coerces int to float)
    pub fn as_float(&self) -> Result<f64, String> {
        match self {
            Value::Float(f) => Ok(*f),
            Value::Int(n) => Ok(*n as f64),
            _ => Err(format!("Expected float or int, got {}", self.type_name())),
        }
    }

    /// Extract as column
    pub fn as_col(&self) -> Result<Arc<blawktrust::Column>, String> {
        match self {
            Value::Col(c) => Ok(Arc::clone(c)),
            _ => Err(format!("Expected col, got {}", self.type_name())),
        }
    }

    /// Extract as table
    pub fn as_table(&self) -> Result<Arc<Table>, String> {
        match self {
            Value::Table(t) => Ok(Arc::clone(t)),
            _ => Err(format!("Expected table, got {}", self.type_name())),
        }
    }

    /// Extract as tableview
    pub fn as_tableview(&self) -> Result<Arc<blawktrust::TableView>, String> {
        match self {
            Value::TableView(tv) => Ok(Arc::clone(tv)),
            _ => Err(format!("Expected tableview, got {}", self.type_name())),
        }
    }

    /// Pretty-print value
    pub fn display(&self, interner: &crate::ast::Interner) -> String {
        match self {
            Value::Nil => "nil".to_string(),
            Value::Bool(b) => b.to_string(),
            Value::Int(n) => n.to_string(),
            Value::Float(f) => f.to_string(),
            Value::Str(s) => format!("\"{}\"", s),
            Value::Sym(id) => format!("'{}", interner.resolve(*id)),
            Value::List(items) => {
                let item_strs: Vec<String> = items.iter()
                    .map(|v| v.display(interner))
                    .collect();
                format!("({})", item_strs.join(" "))
            }
            Value::Col(c) => display_column(c),
            Value::Table(t) => {
                // For display(), we need to return a String, but use write_table_to internally
                // This is used for print builtin which doesn't go to stdout directly
                let mut buf = Vec::new();
                if write_table_to(&mut buf, t, interner, Some(30)).is_ok() {
                    String::from_utf8_lossy(&buf).to_string()
                } else {
                    format!("Table[{} rows × {} cols]", t.row_count, t.columns.len())
                }
            }
            Value::TableView(tv) => {
                // Show orientation and shape
                let (nr, nc) = tv.logical_shape();
                let ori_name = match tv.ori {
                    blawktrust::ORI_H => "H",
                    blawktrust::ORI_Z => "Z",
                    blawktrust::ORI_X => "X",
                    blawktrust::ORI_R => "R",
                    _ => "?",
                };
                format!("TableView[ori={}, shape={}×{}]", ori_name, nr, nc)
            }
        }
    }
}

impl Table {
    /// Create a new empty table
    pub fn new() -> Self {
        Self {
            columns: Vec::new(),
            row_count: 0,
        }
    }

    /// Add a column to the table
    pub fn add_column(&mut self, name: SymbolId, col: blawktrust::Column) {
        let len = col.len();
        if self.columns.is_empty() {
            self.row_count = len;
        }
        self.columns.push((name, col));
    }

    /// Get a column by name
    pub fn get_column(&self, name: SymbolId) -> Option<&blawktrust::Column> {
        self.columns.iter()
            .find(|(n, _)| *n == name)
            .map(|(_, c)| c)
    }
}

impl Default for Table {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_value_types() {
        let nil = Value::Nil;
        let int = Value::Int(42);
        let float = Value::Float(3.14);
        let boolean = Value::Bool(true);

        assert_eq!(nil.type_name(), "nil");
        assert_eq!(int.type_name(), "int");
        assert_eq!(float.type_name(), "float");
        assert_eq!(boolean.type_name(), "bool");
    }

    #[test]
    fn test_is_truthy() {
        assert!(!Value::Nil.is_truthy());
        assert!(!Value::Bool(false).is_truthy());
        assert!(Value::Bool(true).is_truthy());
        assert!(Value::Int(0).is_truthy()); // 0 is truthy in Lisp
        assert!(Value::Int(42).is_truthy());
    }

    #[test]
    fn test_clone() {
        let val = Value::Int(42);
        let cloned = val.clone();
        assert_eq!(val, cloned);
    }

    #[test]
    fn test_as_int() {
        assert_eq!(Value::Int(42).as_int().unwrap(), 42);
        assert!(Value::Float(3.14).as_int().is_err());
    }

    #[test]
    fn test_as_float() {
        assert_eq!(Value::Float(3.14).as_float().unwrap(), 3.14);
        assert_eq!(Value::Int(42).as_float().unwrap(), 42.0); // Coercion
    }

    #[test]
    fn test_col_type() {
        let data = vec![1.0, 2.0, 3.0];
        let col = blawktrust::Column::new_f64(data);
        let val = Value::Col(Arc::new(col));

        assert_eq!(val.type_name(), "col");
        assert!(val.as_col().is_ok());
    }

    #[test]
    fn test_table_type() {
        let table = Table::new();
        let val = Value::Table(Arc::new(table));

        assert_eq!(val.type_name(), "table");
        assert!(val.as_table().is_ok());
    }

    #[test]
    fn test_table_add_column() {
        let mut table = Table::new();
        let col = blawktrust::Column::new_f64(vec![1.0, 2.0, 3.0]);
        let name = SymbolId(0);

        table.add_column(name, col);

        assert_eq!(table.row_count, 3);
        assert_eq!(table.columns.len(), 1);
        assert!(table.get_column(name).is_some());
    }

    #[test]
    fn test_display() {
        let interner = crate::ast::Interner::new();

        assert_eq!(Value::Nil.display(&interner), "nil");
        assert_eq!(Value::Int(42).display(&interner), "42");
        assert_eq!(Value::Float(3.14).display(&interner), "3.14");
        assert_eq!(Value::Bool(true).display(&interner), "true");
        assert_eq!(Value::Str("hello".into()).display(&interner), "\"hello\"");

        let col = blawktrust::Column::new_f64(vec![1.0, 2.0, 3.0]);
        let val = Value::Col(Arc::new(col));
        assert_eq!(val.display(&interner), "F64[1, 2, 3] (n=3)");

        let table = Table::new();
        let val = Value::Table(Arc::new(table));
        assert_eq!(val.display(&interner), "");
    }
}
