//! Runtime values for blisp
//!
//! Step 4: Full implementation with Col and Table types.

use crate::ast::SymbolId;
use std::sync::Arc;

/// Convert days since Unix epoch to YYYY-MM-DD string
fn days_to_date(days: i64) -> String {
    if days == blawktrust::NULL_TS {
        return "NA".to_string();
    }

    // Unix epoch: 1970-01-01
    // Simple algorithm: approximate year, then calculate exact date
    let mut remaining = days;
    let mut year = 1970;

    // Handle negative days (before 1970)
    if remaining < 0 {
        // Each year before 1970 has roughly 365.25 days
        let years_back = (-remaining / 366) + 1;
        year -= years_back;
        // Account for leap years approximately
        remaining += years_back * 365 + (years_back / 4);
    }

    // Find the year
    loop {
        let days_in_year = if is_leap_year(year) { 366 } else { 365 };
        if remaining < days_in_year {
            break;
        }
        remaining -= days_in_year;
        year += 1;
    }

    // Find month and day
    let days_in_months = if is_leap_year(year) {
        [31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    } else {
        [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    };

    let mut month = 1;
    let mut day = remaining + 1; // Days are 1-indexed

    for &days_in_month in &days_in_months {
        if day <= days_in_month {
            break;
        }
        day -= days_in_month;
        month += 1;
    }

    format!("{:04}-{:02}-{:02}", year, month, day)
}

fn is_leap_year(year: i64) -> bool {
    (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0)
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
        blawktrust::Column::Ts(data) => {
            let len = data.len();
            if len == 0 {
                return "Ts[]".to_string();
            }

            let mut parts = vec!["Ts[".to_string()];

            if len <= MAX_SHOW {
                for (i, &val) in data.iter().enumerate() {
                    if i > 0 { parts.push(", ".to_string()); }
                    parts.push(days_to_date(val));
                }
            } else {
                // Show first 10
                for i in 0..10 {
                    if i > 0 { parts.push(", ".to_string()); }
                    parts.push(days_to_date(data[i]));
                }
                parts.push(", ...".to_string());
                // Show last 2
                for i in (len - 2)..len {
                    parts.push(", ".to_string());
                    parts.push(days_to_date(data[i]));
                }
            }

            parts.push(format!("] (n={})", len));
            parts.concat()
        }
    }
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
    Col(Arc<blawktrust::Column>),
    Table(Arc<Table>),
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
            // Columns compare by pointer for now
            (Value::Col(a), Value::Col(b)) => Arc::ptr_eq(a, b),
            (Value::Table(a), Value::Table(b)) => Arc::ptr_eq(a, b),
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
            Value::Col(_) => "col",
            Value::Table(_) => "table",
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

    /// Pretty-print value
    pub fn display(&self, interner: &crate::ast::Interner) -> String {
        match self {
            Value::Nil => "nil".to_string(),
            Value::Bool(b) => b.to_string(),
            Value::Int(n) => n.to_string(),
            Value::Float(f) => f.to_string(),
            Value::Str(s) => format!("\"{}\"", s),
            Value::Sym(id) => format!("'{}", interner.resolve(*id)),
            Value::Col(c) => display_column(c),
            Value::Table(t) => format!("Table[{} rows × {} cols]", t.row_count, t.columns.len()),
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
        assert_eq!(val.display(&interner), "Table[0 rows × 0 cols]");
    }
}
