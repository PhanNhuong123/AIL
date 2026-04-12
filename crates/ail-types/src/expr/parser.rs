//! Recursive-descent parser for constraint and value expressions.
//!
//! Operator precedence (lowest to highest):
//!   1. `or`
//!   2. `and`
//!   3. `not`
//!   4. comparison / `in` / `matches`  (non-associative)
//!   5. additive  `+`, `-`
//!   6. multiplicative  `*`, `/`, `%`
//!   7. primary  (literal, ref, old, call, parenthesised, quantifier)
//!
//! `And` and `Or` are n-ary: consecutive same-operator terms are flattened into
//! a single `Vec` instead of a binary tree.

use std::str::FromStr;

use crate::errors::ParseError;
use crate::types::{ArithOp, CompareOp, ConstraintExpr, LiteralValue, ValueExpr};

// ── Tokens ───────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
enum Token {
    // Literals
    Int(i64),
    Float(f64),
    Str(String),
    /// Regex pattern content (without the surrounding `/` delimiters).
    /// Emitted by the lexer only when `/.../ ` follows the `matches` keyword.
    Regex(String),
    True,
    False,
    Nothing,

    // Identifiers / keywords (resolved after lexing)
    Ident(String),

    // Operators
    Gte,      // >=
    Lte,      // <=
    Gt,       // >
    Lt,       // <
    DoubleEq, // ==
    BangEq,   // !=

    Plus,    // +
    Minus,   // -
    Star,    // *
    Slash,   // /
    Percent, // %

    // Punctuation
    Dot,
    Comma,
    LParen,
    RParen,
    LBrace,
    RBrace,

    // Multi-word / keyword tokens
    And,
    Or,
    Not,
    Is,
    In,
    Matches,
    Old,
    For,
    All,
    Exists,
    Where,
}

// ── Lexer ────────────────────────────────────────────────────────────────────

struct Lexer<'a> {
    src: &'a [u8],
    pos: usize,
}

impl<'a> Lexer<'a> {
    fn new(src: &'a str) -> Self {
        Self { src: src.as_bytes(), pos: 0 }
    }

    fn peek(&self) -> Option<u8> {
        self.src.get(self.pos).copied()
    }

    fn peek2(&self) -> Option<u8> {
        self.src.get(self.pos + 1).copied()
    }

    fn advance(&mut self) -> Option<u8> {
        let ch = self.src.get(self.pos).copied();
        if ch.is_some() {
            self.pos += 1;
        }
        ch
    }

    fn skip_whitespace(&mut self) {
        while matches!(self.peek(), Some(b' ' | b'\t' | b'\r' | b'\n')) {
            self.advance();
        }
    }

    fn lex_string(&mut self, quote: u8) -> Result<Token, ParseError> {
        // quote char already consumed
        let mut s = String::new();
        loop {
            match self.advance() {
                None => return Err(ParseError::UnterminatedString),
                Some(c) if c == quote => break,
                Some(b'\\') => {
                    match self.advance() {
                        Some(b'n') => s.push('\n'),
                        Some(b't') => s.push('\t'),
                        Some(b'\\') => s.push('\\'),
                        Some(b'"') => s.push('"'),
                        Some(b'\'') => s.push('\''),
                        Some(c) => { s.push('\\'); s.push(c as char); }
                        None => return Err(ParseError::UnterminatedString),
                    }
                }
                Some(c) => s.push(c as char),
            }
        }
        Ok(Token::Str(s))
    }

    /// Lex a regex pattern body after the opening `/` has been consumed.
    fn lex_regex(&mut self) -> Result<Token, ParseError> {
        let mut s = String::new();
        loop {
            match self.advance() {
                None => return Err(ParseError::UnterminatedRegex),
                Some(b'/') => break,
                Some(b'\\') => {
                    s.push('\\');
                    match self.advance() {
                        Some(c) => s.push(c as char),
                        None => return Err(ParseError::UnterminatedRegex),
                    }
                }
                Some(c) => s.push(c as char),
            }
        }
        Ok(Token::Regex(s))
    }

    fn lex_number(&mut self, _first: u8) -> Result<Token, ParseError> {
        let start = self.pos - 1;
        let mut has_dot = false;
        while matches!(self.peek(), Some(b'0'..=b'9' | b'.' | b'e' | b'E' | b'+' | b'-')) {
            if self.peek() == Some(b'.') {
                if has_dot {
                    break;
                }
                // Only consume '.' if it looks like a decimal (not a field access)
                if !matches!(self.peek2(), Some(b'0'..=b'9')) {
                    break;
                }
                has_dot = true;
            }
            self.advance();
        }
        let raw = std::str::from_utf8(&self.src[start..self.pos]).unwrap();
        if has_dot {
            raw.parse::<f64>()
                .map(Token::Float)
                .map_err(|_| ParseError::InvalidNumber(raw.to_owned()))
        } else {
            raw.parse::<i64>()
                .map(Token::Int)
                .or_else(|_| raw.parse::<f64>().map(Token::Float))
                .map_err(|_| ParseError::InvalidNumber(raw.to_owned()))
        }
    }

    fn lex_ident(&mut self, _first: u8) -> Token {
        let start = self.pos - 1;
        while matches!(self.peek(), Some(b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'_')) {
            self.advance();
        }
        let word = std::str::from_utf8(&self.src[start..self.pos]).unwrap();
        match word {
            "and" => Token::And,
            "or" => Token::Or,
            "not" => Token::Not,
            "is" => Token::Is,
            "in" => Token::In,
            "matches" => Token::Matches,
            "old" => Token::Old,
            "for" => Token::For,
            "all" => Token::All,
            "exists" => Token::Exists,
            "where" => Token::Where,
            "true" => Token::True,
            "false" => Token::False,
            "nothing" => Token::Nothing,
            _ => Token::Ident(word.to_owned()),
        }
    }

    fn tokenize(mut self) -> Result<Vec<(Token, usize)>, ParseError> {
        let mut tokens = Vec::new();
        let mut last_was_matches = false;
        loop {
            self.skip_whitespace();
            let pos = self.pos;
            match self.peek() {
                None => break,
                Some(c) => {
                    self.advance();
                    let tok = match c {
                        b'(' => Token::LParen,
                        b')' => Token::RParen,
                        b'{' => Token::LBrace,
                        b'}' => Token::RBrace,
                        b',' => Token::Comma,
                        b'.' => Token::Dot,
                        b'+' => Token::Plus,
                        b'-' => {
                            // Negative numbers: only when followed directly by a digit
                            if matches!(self.peek(), Some(b'0'..=b'9')) {
                                self.lex_number(b'-')?
                            } else {
                                Token::Minus
                            }
                        }
                        b'*' => Token::Star,
                        b'%' => Token::Percent,
                        b'/' => {
                            // After `matches` keyword: lex as regex pattern body.
                            // Otherwise: division operator.
                            if last_was_matches {
                                self.lex_regex()?
                            } else {
                                Token::Slash
                            }
                        }
                        b'>' => {
                            if self.peek() == Some(b'=') {
                                self.advance();
                                Token::Gte
                            } else {
                                Token::Gt
                            }
                        }
                        b'<' => {
                            if self.peek() == Some(b'=') {
                                self.advance();
                                Token::Lte
                            } else {
                                Token::Lt
                            }
                        }
                        b'=' => {
                            if self.peek() == Some(b'=') {
                                self.advance();
                                Token::DoubleEq
                            } else {
                                return Err(ParseError::UnexpectedChar('=', pos));
                            }
                        }
                        b'!' => {
                            if self.peek() == Some(b'=') {
                                self.advance();
                                Token::BangEq
                            } else {
                                return Err(ParseError::UnexpectedChar('!', pos));
                            }
                        }
                        b'"' => self.lex_string(b'"')?,
                        b'\'' => self.lex_string(b'\'')?,
                        b'0'..=b'9' => self.lex_number(c)?,
                        b'a'..=b'z' | b'A'..=b'Z' | b'_' => self.lex_ident(c),
                        other => {
                            return Err(ParseError::UnexpectedChar(other as char, pos));
                        }
                    };
                    last_was_matches = tok == Token::Matches;
                    tokens.push((tok, pos));
                }
            }
        }
        Ok(tokens)
    }
}

// ── Parser ───────────────────────────────────────────────────────────────────

struct Parser {
    tokens: Vec<(Token, usize)>,
    pos: usize,
}

impl Parser {
    fn new(tokens: Vec<(Token, usize)>) -> Self {
        Self { tokens, pos: 0 }
    }

    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.pos).map(|(t, _)| t)
    }

    fn peek_pos(&self) -> usize {
        self.tokens.get(self.pos).map(|(_, p)| *p).unwrap_or(usize::MAX)
    }

    fn advance(&mut self) -> Option<&Token> {
        let t = self.tokens.get(self.pos).map(|(t, _)| t);
        if t.is_some() {
            self.pos += 1;
        }
        t
    }

    fn expect_ident(&mut self) -> Result<String, ParseError> {
        match self.peek() {
            Some(Token::Ident(_)) => {
                if let Some(Token::Ident(s)) = self.advance() {
                    Ok(s.clone())
                } else {
                    unreachable!()
                }
            }
            other => {
                let got = format!("{other:?}");
                Err(ParseError::Expected("identifier".to_owned(), got))
            }
        }
    }

    fn expect_token(&mut self, expected: &Token) -> Result<(), ParseError> {
        match self.peek() {
            Some(t) if t == expected => {
                self.advance();
                Ok(())
            }
            other => {
                let got = format!("{other:?}");
                Err(ParseError::Expected(format!("{expected:?}"), got))
            }
        }
    }

    // ── Constraint expression parsing ───────────────────────────────────────

    pub fn parse_constraint(&mut self) -> Result<ConstraintExpr, ParseError> {
        if matches!(self.peek(), Some(Token::For)) {
            return self.parse_forall();
        }
        if matches!(self.peek(), Some(Token::Exists)) {
            return self.parse_exists();
        }
        self.parse_or()
    }

    fn parse_or(&mut self) -> Result<ConstraintExpr, ParseError> {
        let left = self.parse_and()?;
        if !matches!(self.peek(), Some(Token::Or)) {
            return Ok(left);
        }
        let mut terms = match left {
            ConstraintExpr::Or(v) => v,
            other => vec![other],
        };
        while matches!(self.peek(), Some(Token::Or)) {
            self.advance();
            let right = self.parse_and()?;
            match right {
                ConstraintExpr::Or(v) => terms.extend(v),
                other => terms.push(other),
            }
        }
        Ok(ConstraintExpr::Or(terms))
    }

    fn parse_and(&mut self) -> Result<ConstraintExpr, ParseError> {
        let left = self.parse_not()?;
        if !matches!(self.peek(), Some(Token::And)) {
            return Ok(left);
        }
        let mut terms = match left {
            ConstraintExpr::And(v) => v,
            other => vec![other],
        };
        while matches!(self.peek(), Some(Token::And)) {
            self.advance();
            let right = self.parse_not()?;
            match right {
                ConstraintExpr::And(v) => terms.extend(v),
                other => terms.push(other),
            }
        }
        Ok(ConstraintExpr::And(terms))
    }

    fn parse_not(&mut self) -> Result<ConstraintExpr, ParseError> {
        if matches!(self.peek(), Some(Token::Not)) {
            self.advance();
            let inner = self.parse_not()?;
            return Ok(ConstraintExpr::Not(Box::new(inner)));
        }
        self.parse_comparison()
    }

    fn parse_comparison(&mut self) -> Result<ConstraintExpr, ParseError> {
        // Parenthesised constraint — e.g. `(a > 0)`
        if matches!(self.peek(), Some(Token::LParen)) {
            self.advance();
            let inner = self.parse_or()?;
            self.expect_token(&Token::RParen)?;
            return Ok(inner);
        }

        let left = self.parse_additive()?;

        match self.peek().cloned() {
            Some(Token::Gte) => { self.advance(); let r = self.parse_additive()?; Ok(ConstraintExpr::Compare { op: CompareOp::Gte, left: Box::new(left), right: Box::new(r) }) }
            Some(Token::Lte) => { self.advance(); let r = self.parse_additive()?; Ok(ConstraintExpr::Compare { op: CompareOp::Lte, left: Box::new(left), right: Box::new(r) }) }
            Some(Token::Gt)  => { self.advance(); let r = self.parse_additive()?; Ok(ConstraintExpr::Compare { op: CompareOp::Gt,  left: Box::new(left), right: Box::new(r) }) }
            Some(Token::Lt)  => { self.advance(); let r = self.parse_additive()?; Ok(ConstraintExpr::Compare { op: CompareOp::Lt,  left: Box::new(left), right: Box::new(r) }) }
            Some(Token::DoubleEq) => { self.advance(); let r = self.parse_additive()?; Ok(ConstraintExpr::Compare { op: CompareOp::Eq,  left: Box::new(left), right: Box::new(r) }) }
            Some(Token::BangEq)   => { self.advance(); let r = self.parse_additive()?; Ok(ConstraintExpr::Compare { op: CompareOp::Neq, left: Box::new(left), right: Box::new(r) }) }
            Some(Token::Is) => {
                self.advance();
                if matches!(self.peek(), Some(Token::Not)) {
                    self.advance();
                    let r = self.parse_additive()?;
                    Ok(ConstraintExpr::Compare { op: CompareOp::IsNot, left: Box::new(left), right: Box::new(r) })
                } else {
                    let r = self.parse_additive()?;
                    Ok(ConstraintExpr::Compare { op: CompareOp::Is, left: Box::new(left), right: Box::new(r) })
                }
            }
            Some(Token::In) => {
                self.advance();
                let collection = self.parse_in_target()?;
                Ok(ConstraintExpr::In { value: Box::new(left), collection: Box::new(collection) })
            }
            Some(Token::Matches) => {
                self.advance();
                // The lexer emits Token::Regex immediately after Token::Matches.
                match self.advance().cloned() {
                    Some(Token::Regex(pattern)) => {
                        Ok(ConstraintExpr::Matches { value: Box::new(left), pattern })
                    }
                    other => Err(ParseError::Expected(
                        "/pattern/".to_owned(),
                        format!("{other:?}"),
                    )),
                }
            }
            _ => Err(ParseError::Expected(
                "comparison operator".to_owned(),
                format!("{:?}", self.peek()),
            )),
        }
    }

    /// Parse `{a, b, c}` or a plain value expression as the RHS of `in`.
    fn parse_in_target(&mut self) -> Result<ValueExpr, ParseError> {
        if matches!(self.peek(), Some(Token::LBrace)) {
            self.advance();
            let mut elements = Vec::new();
            loop {
                if matches!(self.peek(), Some(Token::RBrace)) {
                    self.advance();
                    break;
                }
                elements.push(self.parse_additive()?);
                match self.peek() {
                    Some(Token::Comma) => { self.advance(); }
                    Some(Token::RBrace) => {}
                    other => {
                        return Err(ParseError::Expected(
                            "',' or '}'".to_owned(),
                            format!("{other:?}"),
                        ));
                    }
                }
            }
            Ok(ValueExpr::Set(elements))
        } else {
            self.parse_additive()
        }
    }

    fn parse_forall(&mut self) -> Result<ConstraintExpr, ParseError> {
        self.advance(); // consume `for`
        self.expect_token(&Token::All)?;
        let variable = self.expect_ident()?;
        self.expect_token(&Token::In)?;
        let collection = self.parse_additive()?;
        self.expect_token(&Token::Comma)?;
        let condition = self.parse_constraint()?;
        Ok(ConstraintExpr::ForAll {
            variable,
            collection: Box::new(collection),
            condition: Box::new(condition),
        })
    }

    fn parse_exists(&mut self) -> Result<ConstraintExpr, ParseError> {
        self.advance(); // consume `exists`
        let variable = self.expect_ident()?;
        self.expect_token(&Token::In)?;
        let collection = self.parse_additive()?;
        self.expect_token(&Token::Where)?;
        let condition = self.parse_constraint()?;
        Ok(ConstraintExpr::Exists {
            variable,
            collection: Box::new(collection),
            condition: Box::new(condition),
        })
    }

    // ── Value expression parsing ─────────────────────────────────────────────

    pub fn parse_value(&mut self) -> Result<ValueExpr, ParseError> {
        self.parse_additive()
    }

    fn parse_additive(&mut self) -> Result<ValueExpr, ParseError> {
        let mut left = self.parse_multiplicative()?;
        loop {
            match self.peek() {
                Some(Token::Plus)  => { self.advance(); let r = self.parse_multiplicative()?; left = ValueExpr::Arithmetic { op: ArithOp::Add, left: Box::new(left), right: Box::new(r) }; }
                Some(Token::Minus) => { self.advance(); let r = self.parse_multiplicative()?; left = ValueExpr::Arithmetic { op: ArithOp::Sub, left: Box::new(left), right: Box::new(r) }; }
                _ => break,
            }
        }
        Ok(left)
    }

    fn parse_multiplicative(&mut self) -> Result<ValueExpr, ParseError> {
        let mut left = self.parse_primary()?;
        loop {
            match self.peek() {
                Some(Token::Star)    => { self.advance(); let r = self.parse_primary()?; left = ValueExpr::Arithmetic { op: ArithOp::Mul, left: Box::new(left), right: Box::new(r) }; }
                Some(Token::Slash)   => { self.advance(); let r = self.parse_primary()?; left = ValueExpr::Arithmetic { op: ArithOp::Div, left: Box::new(left), right: Box::new(r) }; }
                Some(Token::Percent) => { self.advance(); let r = self.parse_primary()?; left = ValueExpr::Arithmetic { op: ArithOp::Mod, left: Box::new(left), right: Box::new(r) }; }
                _ => break,
            }
        }
        Ok(left)
    }

    fn parse_primary(&mut self) -> Result<ValueExpr, ParseError> {
        match self.peek().cloned() {
            Some(Token::Int(n))   => { self.advance(); Ok(ValueExpr::Literal(LiteralValue::Integer(n))) }
            Some(Token::Float(f)) => { self.advance(); Ok(ValueExpr::Literal(LiteralValue::Float(f))) }
            Some(Token::Str(s))   => { self.advance(); Ok(ValueExpr::Literal(LiteralValue::Text(s))) }
            Some(Token::True)     => { self.advance(); Ok(ValueExpr::Literal(LiteralValue::Bool(true))) }
            Some(Token::False)    => { self.advance(); Ok(ValueExpr::Literal(LiteralValue::Bool(false))) }
            Some(Token::Nothing)  => { self.advance(); Ok(ValueExpr::Literal(LiteralValue::Nothing)) }
            Some(Token::Old) => {
                self.advance();
                self.expect_token(&Token::LParen)?;
                let inner = self.parse_additive()?;
                self.expect_token(&Token::RParen)?;
                Ok(ValueExpr::Old(Box::new(inner)))
            }
            Some(Token::LParen) => {
                self.advance();
                let inner = self.parse_additive()?;
                self.expect_token(&Token::RParen)?;
                Ok(inner)
            }
            Some(Token::Ident(name)) => {
                self.advance();
                if matches!(self.peek(), Some(Token::LParen)) {
                    self.advance();
                    let mut args = Vec::new();
                    if !matches!(self.peek(), Some(Token::RParen)) {
                        args.push(self.parse_additive()?);
                        while matches!(self.peek(), Some(Token::Comma)) {
                            self.advance();
                            args.push(self.parse_additive()?);
                        }
                    }
                    self.expect_token(&Token::RParen)?;
                    return Ok(ValueExpr::Call { name, args });
                }
                // Dotted path reference
                let mut path = vec![name];
                while matches!(self.peek(), Some(Token::Dot)) {
                    self.advance();
                    path.push(self.expect_ident()?);
                }
                Ok(ValueExpr::Ref(path))
            }
            other => {
                let pos = self.peek_pos();
                Err(ParseError::Expected(
                    "value expression".to_owned(),
                    format!("{other:?} at position {pos}"),
                ))
            }
        }
    }
}

// ── Public parse functions ───────────────────────────────────────────────────

/// Parse a constraint expression from a string slice.
pub fn parse_constraint_expr(s: &str) -> Result<ConstraintExpr, ParseError> {
    let tokens = Lexer::new(s).tokenize()?;
    let mut parser = Parser::new(tokens);
    let expr = parser.parse_constraint()?;
    if parser.pos < parser.tokens.len() {
        let (_, pos) = &parser.tokens[parser.pos];
        return Err(ParseError::Expected(
            "end of input".to_owned(),
            format!("token at position {pos}"),
        ));
    }
    Ok(expr)
}

/// Parse a value expression from a string slice.
pub fn parse_value_expr(s: &str) -> Result<ValueExpr, ParseError> {
    let tokens = Lexer::new(s).tokenize()?;
    let mut parser = Parser::new(tokens);
    let expr = parser.parse_value()?;
    if parser.pos < parser.tokens.len() {
        let (_, pos) = &parser.tokens[parser.pos];
        return Err(ParseError::Expected(
            "end of input".to_owned(),
            format!("token at position {pos}"),
        ));
    }
    Ok(expr)
}

// ── FromStr implementations ──────────────────────────────────────────────────

impl FromStr for ConstraintExpr {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        parse_constraint_expr(s)
    }
}

impl FromStr for ValueExpr {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        parse_value_expr(s)
    }
}
