//! S-expression reader/parser
#![allow(clippy::doc_lazy_continuation)]

use crate::ast::{Expr, Interner};

#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    LParen,
    RParen,
    Quote,
    QuasiQuote,
    Unquote,
    UnquoteSplicing,
    Int(i64),
    Float(f64),
    Str(String),
    Sym(String),
}

/// Simple tokenizer
pub fn tokenize(input: &str) -> Result<Vec<Token>, String> {
    let mut tokens = Vec::new();
    let mut chars = input.chars().peekable();

    while let Some(&ch) = chars.peek() {
        match ch {
            // Whitespace
            ' ' | '\t' | '\n' | '\r' => {
                chars.next();
            }

            // Comment
            ';' => {
                chars.next();
                // Skip until newline
                while let Some(&ch) = chars.peek() {
                    chars.next();
                    if ch == '\n' {
                        break;
                    }
                }
            }

            // Parens
            '(' => {
                tokens.push(Token::LParen);
                chars.next();
            }
            ')' => {
                tokens.push(Token::RParen);
                chars.next();
            }

            // Quote
            '\'' => {
                tokens.push(Token::Quote);
                chars.next();
            }

            // QuasiQuote
            '`' => {
                tokens.push(Token::QuasiQuote);
                chars.next();
            }

            // Unquote and UnquoteSplicing
            ',' => {
                chars.next();
                if let Some(&'@') = chars.peek() {
                    chars.next();
                    tokens.push(Token::UnquoteSplicing);
                } else {
                    tokens.push(Token::Unquote);
                }
            }

            // String
            '"' => {
                chars.next(); // Skip opening "
                let mut s = String::new();
                while let Some(&ch) = chars.peek() {
                    chars.next();
                    if ch == '"' {
                        break;
                    }
                    if ch == '\\' {
                        // Simple escape handling
                        if let Some(&next) = chars.peek() {
                            chars.next();
                            match next {
                                'n' => s.push('\n'),
                                't' => s.push('\t'),
                                '\\' => s.push('\\'),
                                '"' => s.push('"'),
                                _ => s.push(next),
                            }
                        }
                    } else {
                        s.push(ch);
                    }
                }
                tokens.push(Token::Str(s));
            }

            // Numbers and symbols
            _ => {
                let mut token_str = String::new();
                while let Some(&ch) = chars.peek() {
                    if ch.is_whitespace()
                        || ch == '('
                        || ch == ')'
                        || ch == '\''
                        || ch == '`'
                        || ch == ','
                        || ch == ';'
                    {
                        break;
                    }
                    token_str.push(ch);
                    chars.next();
                }

                // Try to parse as number
                if let Ok(n) = token_str.parse::<i64>() {
                    tokens.push(Token::Int(n));
                } else if let Ok(f) = token_str.parse::<f64>() {
                    tokens.push(Token::Float(f));
                } else {
                    // All remaining tokens are symbols (including nil, t, true, false)
                    tokens.push(Token::Sym(token_str));
                }
            }
        }
    }

    Ok(tokens)
}

/// Reader: converts tokens to AST
pub struct Reader {
    tokens: Vec<Token>,
    pos: usize,
}

impl Reader {
    pub fn new(input: &str) -> Result<Self, String> {
        let tokens = tokenize(input)?;
        Ok(Self { tokens, pos: 0 })
    }

    pub fn from_tokens(tokens: Vec<Token>) -> Self {
        Self { tokens, pos: 0 }
    }

    pub fn is_eof(&self) -> bool {
        self.pos >= self.tokens.len()
    }

    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.pos)
    }

    fn advance(&mut self) -> Option<&Token> {
        let token = self.tokens.get(self.pos);
        self.pos += 1;
        token
    }

    pub fn read(&mut self, interner: &mut Interner) -> Result<Expr, String> {
        let token = self.peek().ok_or("Unexpected EOF")?;

        match token {
            Token::LParen => self.read_list(interner),
            Token::Quote => self.read_quote(interner),
            Token::QuasiQuote => self.read_quasiquote(interner),
            Token::Unquote => self.read_unquote(interner),
            Token::UnquoteSplicing => self.read_unquote_splicing(interner),
            Token::Int(n) => {
                let n = *n;
                self.advance();
                Ok(Expr::Int(n))
            }
            Token::Float(f) => {
                let f = *f;
                self.advance();
                Ok(Expr::Float(f))
            }
            Token::Str(s) => {
                let s = s.clone();
                self.advance();
                Ok(Expr::Str(s))
            }
            Token::Sym(s) => {
                let s = s.clone();
                self.advance();
                if s == "nil" {
                    Ok(Expr::Nil)
                } else if s == "t" || s == "true" {
                    Ok(Expr::Bool(true))
                } else {
                    let id = interner.intern(&s);
                    Ok(Expr::Sym(id))
                }
            }
            Token::RParen => Err("Unexpected ')'".to_string()),
        }
    }

    fn read_list(&mut self, interner: &mut Interner) -> Result<Expr, String> {
        self.advance(); // Skip '('

        let mut exprs = Vec::new();
        loop {
            match self.peek() {
                None => return Err("Unclosed list".to_string()),
                Some(Token::RParen) => {
                    self.advance();
                    return Ok(Expr::List(exprs));
                }
                _ => {
                    exprs.push(self.read(interner)?);
                }
            }
        }
    }

    fn read_quote(&mut self, interner: &mut Interner) -> Result<Expr, String> {
        self.advance(); // Skip '
        let expr = self.read(interner)?;
        Ok(Expr::Quote(Box::new(expr)))
    }

    fn read_quasiquote(&mut self, interner: &mut Interner) -> Result<Expr, String> {
        self.advance(); // Skip `
        let expr = self.read(interner)?;
        Ok(Expr::QuasiQuote(Box::new(expr)))
    }

    fn read_unquote(&mut self, interner: &mut Interner) -> Result<Expr, String> {
        self.advance(); // Skip ,
        let expr = self.read(interner)?;
        Ok(Expr::Unquote(Box::new(expr)))
    }

    fn read_unquote_splicing(&mut self, interner: &mut Interner) -> Result<Expr, String> {
        self.advance(); // Skip ,@
        let expr = self.read(interner)?;
        Ok(Expr::UnquoteSplicing(Box::new(expr)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tokenize() {
        let tokens = tokenize("(+ 1 2)").unwrap();
        assert_eq!(tokens.len(), 5);
        assert_eq!(tokens[0], Token::LParen);
        assert_eq!(tokens[1], Token::Sym("+".to_string()));
        assert_eq!(tokens[2], Token::Int(1));
    }

    #[test]
    fn test_read_simple() {
        let mut interner = Interner::new();
        let mut reader = Reader::new("42").unwrap();
        let expr = reader.read(&mut interner).unwrap();
        assert_eq!(expr, Expr::Int(42));
    }

    #[test]
    fn test_read_list() {
        let mut interner = Interner::new();
        let mut reader = Reader::new("(+ 1 2)").unwrap();
        let expr = reader.read(&mut interner).unwrap();

        match expr {
            Expr::List(exprs) => {
                assert_eq!(exprs.len(), 3);
                assert!(matches!(exprs[0], Expr::Sym(_)));
                assert_eq!(exprs[1], Expr::Int(1));
                assert_eq!(exprs[2], Expr::Int(2));
            }
            _ => panic!("Expected list"),
        }
    }

    #[test]
    fn test_read_quote() {
        let mut interner = Interner::new();
        let mut reader = Reader::new("'foo").unwrap();
        let expr = reader.read(&mut interner).unwrap();

        match expr {
            Expr::Quote(inner) => {
                assert!(matches!(*inner, Expr::Sym(_)));
            }
            _ => panic!("Expected quote"),
        }
    }

    #[test]
    fn test_read_string() {
        let mut interner = Interner::new();
        let mut reader = Reader::new(r#""hello world""#).unwrap();
        let expr = reader.read(&mut interner).unwrap();
        assert_eq!(expr, Expr::Str("hello world".to_string()));
    }

    #[test]
    fn test_comment() {
        let mut interner = Interner::new();
        let mut reader = Reader::new("; comment\n42").unwrap();
        let expr = reader.read(&mut interner).unwrap();
        assert_eq!(expr, Expr::Int(42));
    }
}
