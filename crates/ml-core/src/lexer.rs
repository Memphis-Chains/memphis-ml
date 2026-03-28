// ML-Core Lexer — prosty bez Logos (unikamy konfliktów regex/token)

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TokenKind {
    LParen, RParen, LBracket, RBracket,
    Gate, Read, If, Else, Wait, Log, Let,
    On, Off, Toggle, True, False,
    And, Or, Not, Eq, Gt, Lt,
    String, Number, Atom,
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
            '(' => { chars.next(); tokens.push(Token { kind: TokenKind::LParen, text: "(".into() }); }
            ')' => { chars.next(); tokens.push(Token { kind: TokenKind::RParen, text: ")".into() }); }
            '[' => { chars.next(); tokens.push(Token { kind: TokenKind::LBracket, text: "[".into() }); }
            ']' => { chars.next(); tokens.push(Token { kind: TokenKind::RBracket, text: "]".into() }); }
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
            '-' | '0'..='9' => {
                let mut num = String::new();
                while let Some(&c) = chars.peek() {
                    if c.is_numeric() || c == '.' || c == 'e' || c == 'E' || c == '+' || c == '-' {
                        num.push(c); chars.next();
                    } else { break; }
                }
                tokens.push(Token { kind: TokenKind::Number, text: num });
            }
            ';' => {
                while let Some(&c) = chars.peek() { if c == '\n' { break; } chars.next(); }
            }
            ' ' | '\t' | '\r' | '\n' => { chars.next(); }
            '>' => { chars.next(); tokens.push(Token { kind: TokenKind::Gt, text: ">".into() }); }
            '<' => { chars.next(); tokens.push(Token { kind: TokenKind::Lt, text: "<".into() }); }
            '=' => {
                chars.next();
                if chars.peek() == Some(&'=') { chars.next(); tokens.push(Token { kind: TokenKind::Eq, text: "==".into() }); }
                else { tokens.push(Token { kind: TokenKind::Atom, text: "=".into() }); }
            }
            'a'..='z' | 'A'..='Z' | '_' => {
                let mut s = String::new();
                while let Some(&c) = chars.peek() {
                    if c.is_alphanumeric() || c == '_' || c == '.' || c == ':' || c == '-' { s.push(c); chars.next(); }
                    else { break; }
                }
                let kind = match s.as_str() {
                    "gate" => TokenKind::Gate,
                    "read" => TokenKind::Read,
                    "if" => TokenKind::If,
                    "else" => TokenKind::Else,
                    "wait" => TokenKind::Wait,
                    "log" => TokenKind::Log,
                    "let" => TokenKind::Let,
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
            _ => { chars.next(); }
        }
    }
    tokens.push(Token { kind: TokenKind::Eof, text: "".into() });
    tokens
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tok(src: &str) -> Vec<Token> { tokenize(src) }

    #[test]
    fn gate() {
        let t = tok("(gate garage on)");
        assert_eq!(t[0].kind, TokenKind::LParen);
        assert_eq!(t[1].kind, TokenKind::Gate);
        assert_eq!(t[2].kind, TokenKind::Atom);
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
}
