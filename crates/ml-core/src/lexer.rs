// ML-Core Lexer — manual tokenizer

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TokenKind {
    // Ignored
    Ignored,
    // Structurals
    LParen, RParen, LBracket, RBracket,
    // Keywords
    Gate, Read, If, Else, Wait, Log, Let,
    On, Off, Toggle, True, False,
    And, Or, Not,
    // Comparison
    Eq, Neq, Gt, Lt, Gte, Lte,
    // Arithmetic
    Plus, Minus, Star, Slash, Percent,
    // New keywords
    Fn, While, Set, Begin, Call,
    // Literals
    String, Number, Atom,
    // End
    Eof,
}

#[derive(Debug, Clone)]
pub struct Token {
    pub kind: TokenKind,
    pub text: String,
}

pub fn tokenize(source: &str) -> Vec<Token> {
    let mut chars = source.chars().peekable();
    let mut tokens = Vec::new();

    while let Some(&c) = chars.peek() {
        match c {
            // Whitespace
            ' ' | '\t' | '\r' | '\n' => { chars.next(); }
            // Number BEFORE letters — '3' would match 'a'..'z' arm otherwise
            '0'..='9' => {
                let mut num = String::new();
                while let Some(&c) = chars.peek() {
                    if c.is_numeric() || c == '.' || c == 'e' || c == 'E' { num.push(c); chars.next(); }
                    else { break; }
                }
                tokens.push(Token { kind: TokenKind::Number, text: num });
            }
            '"' => {
                chars.next();
                let mut s = String::new();
                while let Some(&c) = chars.peek() {
                    if c == '"' { chars.next(); break; }
                    if c == '\\' { chars.next(); if let Some(e) = chars.next() { s.push(e); } }
                    else { s.push(c); chars.next(); }
                }
                tokens.push(Token { kind: TokenKind::String, text: format!("\"{}\"", s) });
            }
            // Comments
            ';' => {
                while let Some(&c) = chars.peek() { if c == '\n' { break; } chars.next(); }
            }
            // Operators
            '+' => { chars.next(); tokens.push(Token { kind: TokenKind::Plus, text: "+".into() }); }
            '-' => { chars.next(); tokens.push(Token { kind: TokenKind::Minus, text: "-".into() }); }
            '*' => { chars.next(); tokens.push(Token { kind: TokenKind::Star, text: "*".into() }); }
            '/' => { chars.next(); tokens.push(Token { kind: TokenKind::Slash, text: "/".into() }); }
            '%' => { chars.next(); tokens.push(Token { kind: TokenKind::Percent, text: "%".into() }); }
            '=' => {
                chars.next();
                if chars.peek() == Some(&'=') { chars.next(); tokens.push(Token { kind: TokenKind::Eq, text: "==".into() }); }
                else { tokens.push(Token { kind: TokenKind::Atom, text: "=".into() }); }
            }
            '!' => {
                chars.next();
                if chars.peek() == Some(&'=') { chars.next(); tokens.push(Token { kind: TokenKind::Neq, text: "!=".into() }); }
                else { tokens.push(Token { kind: TokenKind::Not, text: "!".into() }); }
            }
            '>' => {
                chars.next();
                if chars.peek() == Some(&'=') { chars.next(); tokens.push(Token { kind: TokenKind::Gte, text: ">=".into() }); }
                else { tokens.push(Token { kind: TokenKind::Gt, text: ">".into() }); }
            }
            '<' => {
                chars.next();
                if chars.peek() == Some(&'=') { chars.next(); tokens.push(Token { kind: TokenKind::Lte, text: "<=".into() }); }
                else { tokens.push(Token { kind: TokenKind::Lt, text: "<".into() }); }
            }
            '&' => {
                chars.next();
                if chars.peek() == Some(&'&') { chars.next(); tokens.push(Token { kind: TokenKind::And, text: "&&".into() }); }
                else { chars.next(); } // skip lone &
            }
            '|' => {
                chars.next();
                if chars.peek() == Some(&'|') { chars.next(); tokens.push(Token { kind: TokenKind::Or, text: "||".into() }); }
                else { chars.next(); } // skip lone |
            }
            // Keywords (AFTER numbers — so '3' is parsed as number, not Atom)
            'a'..='z' | 'A'..='Z' | '_' => {
                let mut s = String::new();
                while let Some(&c) = chars.peek() {
                    if c.is_alphabetic() || c == '_' || c.is_numeric() || c == '.' || c == ':' || c == '-' { s.push(c); chars.next(); }
                    else { break; }
                }
                let kind = match s.as_str() {
                    "gate" => TokenKind::Gate,
                    "read" => TokenKind::Read,
                    "if" => TokenKind::If,
                    "else" => TokenKind::Else,
                    "while" => TokenKind::While,
                    "wait" => TokenKind::Wait,
                    "log" => TokenKind::Log,
                    "let" => TokenKind::Let,
                    "fn" => TokenKind::Fn,
                    "set" => TokenKind::Set,
                    "begin" => TokenKind::Begin,
                    "call" => TokenKind::Call,
                    "on" => TokenKind::On,
                    "off" => TokenKind::Off,
                    "toggle" => TokenKind::Toggle,
                    "true" => TokenKind::True,
                    "false" => TokenKind::False,
                    "and" => TokenKind::And,
                    "or" => TokenKind::Or,
                    "not" => TokenKind::Not,
                    _ => TokenKind::Atom,
                };
                tokens.push(Token { kind, text: s });
            }
            '(' => { chars.next(); tokens.push(Token { kind: TokenKind::LParen, text: "(".into() }); }
            ')' => { chars.next(); tokens.push(Token { kind: TokenKind::RParen, text: ")".into() }); }
            '[' => { chars.next(); tokens.push(Token { kind: TokenKind::LBracket, text: "[".into() }); }
            ']' => { chars.next(); tokens.push(Token { kind: TokenKind::RBracket, text: "]".into() }); }
            _ => { chars.next(); }
        }
    }
    tokens.push(Token { kind: TokenKind::Eof, text: "".into() });
    tokens
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tok(source: &str) -> Vec<Token> { tokenize(source) }

    #[test]
    fn gate() {
        let t = tok("(gate garage on)");
        assert_eq!(t[0].kind, TokenKind::LParen);
        assert_eq!(t[1].kind, TokenKind::Gate);
        assert_eq!(t[1].text, "gate");
        assert_eq!(t[2].kind, TokenKind::Atom);
        assert_eq!(t[2].text, "garage");
        assert_eq!(t[3].kind, TokenKind::On);
    }

    #[test]
    fn read_temp() {
        let t = tok("(read temp.living_room)");
        assert_eq!(t[1].kind, TokenKind::Read);
        assert_eq!(t[2].text, "temp.living_room");
    }

    #[test]
    fn number() {
        let t = tok("(wait 500)");
        assert_eq!(t[2].kind, TokenKind::Number);
        assert_eq!(t[2].text, "500");
    }

    #[test]
    fn string() {
        let t = tok(r#"(log "hello")"#);
        assert_eq!(t[2].kind, TokenKind::String);
        assert_eq!(t[2].text, "\"hello\"");
    }

    #[test]
    fn comparison_ops() {
        let t = tok("(< x 10)");
        assert_eq!(t[1].kind, TokenKind::Lt);
    }

    #[test]
    fn arithmetic_ops() {
        let t = tok("(+ x 1)");
        assert_eq!(t[1].kind, TokenKind::Plus);
        let t2 = tok("(- x 2)");
        assert_eq!(t2[1].kind, TokenKind::Minus);
        let t3 = tok("(* x 3)");
        assert_eq!(t3[1].kind, TokenKind::Star);
        let t4 = tok("(/ x 4)");
        assert_eq!(t4[1].kind, TokenKind::Slash);
        let t5 = tok("(% x 5)");
        assert_eq!(t5[1].kind, TokenKind::Percent);
    }
}
