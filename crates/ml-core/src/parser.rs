// ML-Core Parser — minimal recursive descent

use crate::ast::MLExpr;
use crate::error::ParseError;
use crate::lexer::{tokenize, Token, TokenKind};

pub struct Parser {
    tokens: Vec<Token>,
    pos: usize,
}

impl Parser {
    pub fn new(source: &str) -> Self {
        Self { tokens: tokenize(source), pos: 0 }
    }

    pub fn parse(&mut self) -> Result<MLExpr, ParseError> {
        let first = self.parse_expr()?;
        // If there's more after this (ignoring EOF), collect into Sequence
        if self.pos < self.tokens.len() - 1 {
            let mut exprs = vec![first];
            while self.pos < self.tokens.len() - 1 {
                exprs.push(self.parse_expr()?);
            }
            Ok(MLExpr::Sequence(exprs))
        } else {
            Ok(first)
        }
    }

    fn current(&self) -> Option<&Token> { self.tokens.get(self.pos) }

    fn advance(&mut self) { if self.pos < self.tokens.len() { self.pos += 1; } }

    fn expect(&mut self, kind: TokenKind) -> Result<Token, ParseError> {
        let tok = self.current().cloned().ok_or(ParseError::UnexpectedEof)?;
        if tok.kind == kind { self.advance(); Ok(tok) }
        else { Err(ParseError::UnexpectedToken(tok.text)) }
    }

    fn peek(&self) -> Option<TokenKind> { self.current().map(|t| t.kind) }

    fn parse_expr(&mut self) -> Result<MLExpr, ParseError> {
        let tok = self.current().cloned().ok_or(ParseError::UnexpectedEof)?;
        match tok.kind {
            TokenKind::Number => {
                self.advance();
                let n: f64 = tok.text.parse().map_err(|_| ParseError::InvalidNumber(tok.text))?;
                Ok(MLExpr::Number(n))
            }
            TokenKind::String => {
                self.advance();
                let s = tok.text.trim_matches('"').to_string();
                Ok(MLExpr::String(s))
            }
            TokenKind::True => { self.advance(); Ok(MLExpr::Bool(true)) }
            TokenKind::False => { self.advance(); Ok(MLExpr::Bool(false)) }
            TokenKind::Atom => { self.advance(); Ok(MLExpr::Var(tok.text)) }
            TokenKind::LParen => self.parse_list(),
            TokenKind::LBracket => self.parse_sequence(),
            _ => Err(ParseError::UnexpectedToken(tok.text)),
        }
    }

    fn parse_sequence(&mut self) -> Result<MLExpr, ParseError> {
        self.advance(); // consume '['
        let mut exprs = Vec::new();
        loop {
            match self.peek() {
                Some(TokenKind::RBracket) | None => break,
                _ => exprs.push(self.parse_expr()?),
            }
        }
        self.expect(TokenKind::RBracket)?;
        Ok(MLExpr::Sequence(exprs))
    }

    fn parse_list(&mut self) -> Result<MLExpr, ParseError> {
        self.advance(); // consume '('
        let kind = self.peek().ok_or(ParseError::UnexpectedEof)?;
        match kind {
            TokenKind::Gate => self.parse_gate(),
            TokenKind::Read => self.parse_read(),
            TokenKind::Wait => self.parse_wait(),
            TokenKind::Log => self.parse_log(),
            TokenKind::Let => self.parse_let(),
            TokenKind::Set => self.parse_set(),
            TokenKind::If => self.parse_if(),
            TokenKind::While => self.parse_while(),
            TokenKind::Fn => self.parse_fn(),
            TokenKind::Begin => self.parse_begin(),
            TokenKind::Plus | TokenKind::Minus | TokenKind::Star | TokenKind::Slash
            | TokenKind::Percent | TokenKind::Eq | TokenKind::Neq
            | TokenKind::Gt | TokenKind::Lt | TokenKind::Gte | TokenKind::Lte
            | TokenKind::And | TokenKind::Or | TokenKind::Not => self.parse_binary_op(),
            TokenKind::Atom => {
                let name = self.current().unwrap().text.clone();
                self.advance();
                let mut args = Vec::new();
                loop {
                    if let Some(TokenKind::RParen) = self.peek() { self.advance(); break; }
                    if self.peek() == None { return Err(ParseError::UnclosedParen); }
                    args.push(self.parse_expr()?);
                }
                Ok(MLExpr::Call { name, args })
            }
            _ => Err(ParseError::UnexpectedToken(self.current().unwrap().text.clone())),
        }
    }

    fn parse_binary_op(&mut self) -> Result<MLExpr, ParseError> {
        let op = match self.current().cloned().ok_or(ParseError::UnexpectedEof)?.kind {
            TokenKind::Plus => "+".into(),
            TokenKind::Minus => "-".into(),
            TokenKind::Star => "*".into(),
            TokenKind::Slash => "/".into(),
            TokenKind::Percent => "%".into(),
            TokenKind::Eq => "==".into(),
            TokenKind::Neq => "!=".into(),
            TokenKind::Gt => ">".into(),
            TokenKind::Lt => "<".into(),
            TokenKind::Gte => ">=".into(),
            TokenKind::Lte => "<=".into(),
            TokenKind::And => "and".into(),
            TokenKind::Or => "or".into(),
            TokenKind::Not => "not".into(),
            _ => return Err(ParseError::UnexpectedToken(self.current().unwrap().text.clone())),
        };
        self.advance();
        let left = Box::new(self.parse_expr()?);
        let right = Box::new(self.parse_expr()?);
        self.expect(TokenKind::RParen)?;
        Ok(MLExpr::BinaryOp { op, left, right })
    }

    fn parse_gate(&mut self) -> Result<MLExpr, ParseError> {
        self.advance(); // consume 'gate'
        let id = match self.current().cloned().ok_or(ParseError::UnexpectedEof)? {
            Token { kind: TokenKind::Atom, text } => { self.advance(); text }
            t => return Err(ParseError::UnexpectedToken(t.text)),
        };
        let state = match self.current().cloned().ok_or(ParseError::UnexpectedEof)?.kind {
            TokenKind::On => { self.advance(); "on".into() }
            TokenKind::Off => { self.advance(); "off".into() }
            TokenKind::Toggle => { self.advance(); "toggle".into() }
            _ => return Err(ParseError::UnexpectedToken(self.current().unwrap().text.clone())),
        };
        self.expect(TokenKind::RParen)?;
        Ok(MLExpr::Gate { id, state })
    }

    fn parse_read(&mut self) -> Result<MLExpr, ParseError> {
        self.advance(); // consume 'read'
        let sensor = match self.current().cloned().ok_or(ParseError::UnexpectedEof)? {
            Token { kind: TokenKind::Atom, text } => { self.advance(); text }
            t => return Err(ParseError::UnexpectedToken(t.text)),
        };
        self.expect(TokenKind::RParen)?;
        Ok(MLExpr::Read { sensor })
    }

    fn parse_wait(&mut self) -> Result<MLExpr, ParseError> {
        self.advance(); // consume 'wait'
        let ms = match self.current().cloned().ok_or(ParseError::UnexpectedEof)? {
            Token { kind: TokenKind::Number, text } => {
                let n: f64 = text.parse().map_err(|_| ParseError::InvalidNumber(text))?;
                self.advance();
                n as u64
            }
            t => return Err(ParseError::UnexpectedToken(t.text)),
        };
        self.expect(TokenKind::RParen)?;
        Ok(MLExpr::Wait { ms })
    }

    fn parse_log(&mut self) -> Result<MLExpr, ParseError> {
        self.advance(); // consume 'log'
        // Log can take a string literal OR a variable reference (atom)
        let inner = self.parse_expr()?;
        self.expect(TokenKind::RParen)?;
        Ok(MLExpr::Log { message: Box::new(inner) })
    }

    fn parse_let(&mut self) -> Result<MLExpr, ParseError> {
        self.advance(); // consume 'let'
        let name = match self.current().cloned().ok_or(ParseError::UnexpectedEof)? {
            Token { kind: TokenKind::Atom, text } => { self.advance(); text }
            t => return Err(ParseError::UnexpectedToken(t.text)),
        };
        let value = Box::new(self.parse_expr()?);
        let body = Box::new(self.parse_expr()?);
        self.expect(TokenKind::RParen)?;
        Ok(MLExpr::Let { name, value, body })
    }

    fn parse_set(&mut self) -> Result<MLExpr, ParseError> {
        self.advance(); // consume 'set'
        let name = match self.current().cloned().ok_or(ParseError::UnexpectedEof)? {
            Token { kind: TokenKind::Atom, text } => { self.advance(); text }
            t => return Err(ParseError::UnexpectedToken(t.text)),
        };
        let value = Box::new(self.parse_expr()?);
        self.expect(TokenKind::RParen)?;
        Ok(MLExpr::Set { name, value })
    }

    fn parse_if(&mut self) -> Result<MLExpr, ParseError> {
        self.advance(); // consume 'if'
        let condition = Box::new(self.parse_expr()?);
        let then_branch = Box::new(self.parse_expr()?);
        let else_ = if let Some(TokenKind::RParen) = self.peek() { None }
            else { Some(Box::new(self.parse_expr()?)) };
        self.expect(TokenKind::RParen)?;
        Ok(MLExpr::If { condition, then_branch, else_ })
    }

    fn parse_while(&mut self) -> Result<MLExpr, ParseError> {
        self.advance(); // consume 'while'
        let condition = Box::new(self.parse_expr()?);
        let body = Box::new(self.parse_expr()?);
        self.expect(TokenKind::RParen)?;
        Ok(MLExpr::While { condition, body })
    }

    fn parse_fn(&mut self) -> Result<MLExpr, ParseError> {
        self.advance(); // consume 'fn'
        // Check if next token is a name (atom before LParen means named defn)
        let name = if let Some(TokenKind::Atom) = self.peek() {
            let n = self.current().unwrap().text.clone();
            self.advance();
            Some(n)
        } else {
            None
        };
        self.expect(TokenKind::LParen)?;
        let mut args = Vec::new();
        loop {
            match self.current().cloned().ok_or(ParseError::UnexpectedEof)? {
                Token { kind: TokenKind::RParen, .. } => { self.advance(); break; }
                Token { kind: TokenKind::Atom, text } => { self.advance(); args.push(text); }
                t => return Err(ParseError::UnexpectedToken(t.text)),
            }
        }
        let body = Box::new(self.parse_expr()?);
        self.expect(TokenKind::RParen)?;
        match name {
            Some(n) => Ok(MLExpr::Defn { name: n, args, body }),
            None => Ok(MLExpr::Fn { args, body }),
        }
    }

    fn parse_begin(&mut self) -> Result<MLExpr, ParseError> {
        self.advance(); // consume 'begin'
        let mut exprs = Vec::new();
        loop {
            if let Some(TokenKind::RParen) = self.peek() { self.advance(); break; }
            if self.peek() == None { return Err(ParseError::UnclosedParen); }
            exprs.push(self.parse_expr()?);
        }
        Ok(MLExpr::Begin(exprs))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(source: &str) -> MLExpr {
        Parser::new(source).parse().unwrap()
    }

    #[test] fn gate_on() { parse("(gate garage on)"); }
    #[test] fn read_temp() { parse("(read temp.living_room)"); }
    #[test] fn wait_ms() { parse("(wait 500)"); }
    #[test] fn if_temp() { parse("(if (> temp 30) (gate cooling on))"); }
    #[test] fn let_binding() {
        let expr = parse("(let x 1 x)");
        match expr { MLExpr::Let { name, .. } => assert_eq!(name, "x"), _ => panic!("expected Let") }
    }
    #[test] fn binary_plus() {
        let expr = parse("(+ x 3)");
        match expr { MLExpr::BinaryOp { op, .. } => assert_eq!(op, "+"), _ => panic!("expected BinaryOp") }
    }
    #[test] fn nested_let() {
        let expr = parse("(let x 5 (+ x 3))");
        match expr {
            MLExpr::Let { body, .. } => match *body {
                MLExpr::BinaryOp { op, .. } => assert_eq!(op, "+"),
                _ => panic!("expected BinaryOp"),
            },
            _ => panic!("expected Let"),
        }
    }
}
