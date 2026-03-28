// ML-Core Parser — recursive descent dla ML
// Obsługuje: gate, read, if, wait, log, let, sekwencje

use crate::ast::*;
use crate::lexer::{tokenize, Token, TokenKind};
use crate::error::ParseError;

pub struct Parser {
    tokens: Vec<Token>,
    pos: usize,
}

impl Parser {
    pub fn new(source: &str) -> Self {
        let tokens = tokenize(source);
        Parser { tokens, pos: 0 }
    }

    pub fn parse(&mut self) -> Result<MLExpr, ParseError> {
        self.parse_sequence()
    }

    fn current(&self) -> Option<&Token> {
        self.tokens.get(self.pos)
    }

    fn advance(&mut self) {
        self.pos += 1;
    }

    fn expect(&mut self, kind: TokenKind) -> Result<Token, ParseError> {
        let tok = self.current().cloned().ok_or(ParseError::UnexpectedEof)?;
        if tok.kind != kind {
            return Err(ParseError::UnexpectedToken(format!(
                "expected {:?}, got {:?}", kind, tok.kind
            )));
        }
        self.advance();
        Ok(tok)
    }

    fn at(&self, kind: TokenKind) -> bool {
        self.current().map(|t| t.kind == kind).unwrap_or(false)
    }

    fn parse_sequence(&mut self) -> Result<MLExpr, ParseError> {
        let mut exprs = Vec::new();
        while !self.at(TokenKind::Eof) {
            exprs.push(self.parse_expr()?);
        }
        if exprs.is_empty() {
            return Err(ParseError::EmptyExpr);
        }
        if exprs.len() == 1 {
            Ok(exprs.remove(0))
        } else {
            Ok(MLExpr::Sequence(exprs))
        }
    }

    fn parse_expr(&mut self) -> Result<MLExpr, ParseError> {
        let tok = self.current().ok_or(ParseError::UnexpectedEof)?.clone();
        match tok.kind {
            TokenKind::LParen => self.parse_list(),
            TokenKind::Atom => {
                self.advance();
                Ok(MLExpr::Var(tok.text))
            }
            TokenKind::String => {
                self.advance();
                let s = tok.text.trim_matches('"');
                Ok(MLExpr::String(s.to_string()))
            }
            TokenKind::Number => {
                self.advance();
                let n: f64 = tok.text.parse().map_err(|_| ParseError::InvalidNumber(tok.text))?;
                Ok(MLExpr::Number(n))
            }
            TokenKind::True => { self.advance(); Ok(MLExpr::Bool(true)) }
            TokenKind::False => { self.advance(); Ok(MLExpr::Bool(false)) }
            _ => Err(ParseError::UnexpectedToken(format!("{:?}", tok.kind))),
        }
    }

    fn parse_list(&mut self) -> Result<MLExpr, ParseError> {
        self.expect(TokenKind::LParen)?;
        let head = self.current().ok_or(ParseError::UnexpectedEof)?.clone();
        self.advance();

        match head.kind {
            TokenKind::Gate => self.parse_gate(),
            TokenKind::Read => self.parse_read(),
            TokenKind::If => self.parse_if(),
            TokenKind::Wait => self.parse_wait(),
            TokenKind::Log => self.parse_log(),
            TokenKind::Let => self.parse_let(),
            _ => self.parse_sequence_inside_paren(),
        }
    }

    fn parse_sequence_inside_paren(&mut self) -> Result<MLExpr, ParseError> {
        let mut exprs = Vec::new();
        while !self.at(TokenKind::RParen) && !self.at(TokenKind::Eof) {
            exprs.push(self.parse_expr()?);
        }
        self.expect(TokenKind::RParen)?;
        if exprs.is_empty() {
            return Err(ParseError::EmptyExpr);
        }
        if exprs.len() == 1 {
            Ok(exprs.remove(0))
        } else {
            Ok(MLExpr::Sequence(exprs))
        }
    }

    fn parse_gate(&mut self) -> Result<MLExpr, ParseError> {
        let id = self.expect(TokenKind::Atom)?.text.clone();
        let state = if self.at(TokenKind::On) {
            self.advance();
            GateState::On
        } else if self.at(TokenKind::Off) {
            self.advance();
            GateState::Off
        } else if self.at(TokenKind::Toggle) {
            self.advance();
            GateState::Toggle
        } else {
            return Err(ParseError::UnexpectedToken("on/off/toggle".to_string()));
        };
        self.expect(TokenKind::RParen)?;
        Ok(MLExpr::Gate { id, state })
    }

    fn parse_read(&mut self) -> Result<MLExpr, ParseError> {
        let sensor = self.expect(TokenKind::Atom)?.text.clone();
        self.expect(TokenKind::RParen)?;
        Ok(MLExpr::Read { sensor })
    }

    fn parse_if(&mut self) -> Result<MLExpr, ParseError> {
        let condition = self.parse_condition()?;
        let then_branch = self.parse_expr()?;
        let else_branch = if self.at(TokenKind::Else) {
            self.advance();
            Some(Box::new(self.parse_expr()?))
        } else {
            None
        };
        self.expect(TokenKind::RParen)?;
        Ok(MLExpr::If { condition, then_branch: Box::new(then_branch), else_: else_branch })
    }

    fn parse_condition(&mut self) -> Result<Condition, ParseError> {
        let tok = self.current().ok_or(ParseError::UnexpectedEof)?.clone();
        match tok.kind {
            TokenKind::True => { self.advance(); Ok(Condition::Bool(true)) }
            TokenKind::False => { self.advance(); Ok(Condition::Bool(false)) }
            TokenKind::Not => {
                self.advance();
                let inner = Box::new(self.parse_condition()?);
                Ok(Condition::Not(inner))
            }
            TokenKind::And => {
                self.advance();
                let left = Box::new(self.parse_condition()?);
                let right = Box::new(self.parse_condition()?);
                Ok(Condition::And(left, right))
            }
            TokenKind::Or => {
                self.advance();
                let left = Box::new(self.parse_condition()?);
                let right = Box::new(self.parse_condition()?);
                Ok(Condition::Or(left, right))
            }
            TokenKind::Eq => {
                self.advance();
                let left = self.parse_value()?;
                let right = self.parse_value()?;
                Ok(Condition::Eq(Box::new(left), Box::new(right)))
            }
            TokenKind::Gt => {
                self.advance();
                let left = self.parse_value()?;
                let right = self.parse_value()?;
                Ok(Condition::Gt(Box::new(left), Box::new(right)))
            }
            TokenKind::Lt => {
                self.advance();
                let left = self.parse_value()?;
                let right = self.parse_value()?;
                Ok(Condition::Lt(Box::new(left), Box::new(right)))
            }
            TokenKind::LParen => {
                // Nested condition: (> x 25) or (and x y)
                self.advance(); // consume '('
                let inner = self.parse_condition()?;
                self.expect(TokenKind::RParen)?;
                Ok(inner)
            }
            _ => {
                let v = self.parse_value()?;
                Ok(Condition::from_value(v))
            }
        }
    }

    fn parse_value(&mut self) -> Result<MLValue, ParseError> {
        let tok = self.current().ok_or(ParseError::UnexpectedEof)?.clone();
        match tok.kind {
            TokenKind::Number => {
                self.advance();
                let n: f64 = tok.text.parse().map_err(|_| ParseError::InvalidNumber(tok.text))?;
                Ok(MLValue::Number(n))
            }
            TokenKind::String => {
                self.advance();
                Ok(MLValue::String(tok.text.trim_matches('"').to_string()))
            }
            TokenKind::Atom => {
                self.advance();
                let s = tok.text;
                if s.starts_with("temp.") {
                    Ok(MLValue::Sensor(s))
                } else if s.starts_with("gate.") {
                    Ok(MLValue::Gate(s))
                } else {
                    Ok(MLValue::Var(s))
                }
            }
            _ => Err(ParseError::UnexpectedToken(format!("value", ))),
        }
    }

    fn parse_wait(&mut self) -> Result<MLExpr, ParseError> {
        let tok = self.expect(TokenKind::Number)?;
        let ms: u64 = tok.text.parse().map_err(|_| ParseError::InvalidNumber(tok.text.clone()))?;
        self.expect(TokenKind::RParen)?;
        Ok(MLExpr::Wait { ms })
    }

    fn parse_log(&mut self) -> Result<MLExpr, ParseError> {
        let tok = self.expect(TokenKind::String)?;
        let msg = tok.text.trim_matches('"').to_string();
        self.expect(TokenKind::RParen)?;
        Ok(MLExpr::Log { message: msg })
    }

    fn parse_let(&mut self) -> Result<MLExpr, ParseError> {
        let name = self.expect(TokenKind::Atom)?.text.clone();
        let value = self.parse_expr()?;
        let body = self.parse_expr()?;
        self.expect(TokenKind::RParen)?;
        Ok(MLExpr::Let { name, value: Box::new(value), body: Box::new(body) })
    }
}

impl Condition {
    fn from_value(v: MLValue) -> Self {
        match v {
            MLValue::Number(n) => Condition::Bool(n != 0.0),
            MLValue::String(s) => Condition::Bool(!s.is_empty()),
            MLValue::Var(_) | MLValue::Sensor(_) | MLValue::Gate(_) => Condition::Bool(true),
            MLValue::Bool(b) => Condition::Bool(b),
            MLValue::Unit => Condition::Bool(false),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(source: &str) -> MLExpr {
        let mut p = Parser::new(source);
        p.parse().unwrap()
    }

    #[test]
    fn gate_on() {
        let expr = parse("(gate garage on)");
        match expr {
            MLExpr::Gate { id, state } => {
                assert_eq!(id, "garage");
                assert_eq!(state, GateState::On);
            }
            _ => panic!("expected Gate, got {:?}", expr),
        }
    }

    #[test]
    fn gate_off() {
        let expr = parse("(gate door off)");
        match expr {
            MLExpr::Gate { id, state } => {
                assert_eq!(id, "door");
                assert_eq!(state, GateState::Off);
            }
            _ => panic!("expected Gate"),
        }
    }

    #[test]
    fn read_temp() {
        let expr = parse("(read temp.living_room)");
        match expr {
            MLExpr::Read { sensor } => assert_eq!(sensor, "temp.living_room"),
            _ => panic!("expected Read"),
        }
    }

    #[test]
    fn wait_ms() {
        let expr = parse("(wait 500)");
        match expr {
            MLExpr::Wait { ms } => assert_eq!(ms, 500),
            _ => panic!("expected Wait"),
        }
    }

    #[test]
    fn sequence() {
        let expr = parse("(gate garage on) (read temp.living_room)");
        match expr {
            MLExpr::Sequence(v) => assert_eq!(v.len(), 2),
            _ => panic!("expected Sequence"),
        }
    }

    #[test]
    fn if_temp() {
        let source = "(if (> temp.living_room 25) (gate fan on))";
        let mut p = Parser::new(source);
        // First expr should be If
        let result = p.parse();
        match result {
            Ok(MLExpr::If { condition, .. }) => {
                eprintln!("OK: condition = {:?}", condition);
            }
            Ok(other) => panic!("expected If, got {:?}", other),
            Err(e) => panic!("parse error: {} (token[2] = {:?})", e, crate::tokenize(source).get(2)),
        }
    }
}
