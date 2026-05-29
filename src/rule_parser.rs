use std::fmt::{self, Display};

use crate::rule::*;

/// Parse a Traefik match rule into an AST.
pub fn parse_rule(input: &str) -> Result<Rule, ParseError> {
    let tokens = tokenize(input)?;
    let mut parser = Parser { tokens, pos: 0 };
    let rule = parser.parse_or()?;
    if let Some(extra) = parser.peek() {
        return Err(ParseError::new(format!("unexpected trailing {extra:?}")));
    }
    Ok(rule)
}

/// An error produced while tokenizing or parsing a match rule.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseError {
    pub message: String,
}

impl ParseError {
    fn new(message: impl Into<String>) -> Self {
        ParseError {
            message: message.into(),
        }
    }
}

impl std::error::Error for ParseError {}

impl Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invalid match rule: {}", self.message)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Token {
    Ident(String),
    Value(String),
    LParen,
    RParen,
    Comma,
    And,
    Or,
    Not,
}

fn tokenize(input: &str) -> Result<Vec<Token>, ParseError> {
    let mut tokens = Vec::new();
    let mut chars = input.char_indices().peekable();

    while let Some(&(i, c)) = chars.peek() {
        match c {
            c if c.is_whitespace() => {
                chars.next();
            }
            '(' => {
                chars.next();
                tokens.push(Token::LParen);
            }
            ')' => {
                chars.next();
                tokens.push(Token::RParen);
            }
            ',' => {
                chars.next();
                tokens.push(Token::Comma);
            }
            '!' => {
                chars.next();
                tokens.push(Token::Not);
            }
            '&' => {
                chars.next();
                match chars.next() {
                    Some((_, '&')) => tokens.push(Token::And),
                    _ => return Err(ParseError::new(format!("expected `&&` at position {i}"))),
                }
            }
            '|' => {
                chars.next();
                match chars.next() {
                    Some((_, '|')) => tokens.push(Token::Or),
                    _ => return Err(ParseError::new(format!("expected `||` at position {i}"))),
                }
            }
            '`' => {
                chars.next(); // opening backtick
                let mut value = String::new();
                loop {
                    match chars.next() {
                        Some((_, '`')) => break,
                        Some((_, ch)) => value.push(ch),
                        None => {
                            return Err(ParseError::new(format!(
                                "unterminated backtick value starting at position {i}"
                            )));
                        }
                    }
                }
                tokens.push(Token::Value(value));
            }
            '"' => {
                chars.next(); // opening quote
                let mut value = String::new();
                loop {
                    match chars.next() {
                        Some((_, '"')) => break,
                        // Traefik allows escaped double quotes inside values.
                        Some((_, '\\')) => match chars.next() {
                            Some((_, escaped)) => value.push(escaped),
                            None => {
                                return Err(ParseError::new(format!(
                                    "unterminated escape in value starting at position {i}"
                                )));
                            }
                        },
                        Some((_, ch)) => value.push(ch),
                        None => {
                            return Err(ParseError::new(format!(
                                "unterminated quoted value starting at position {i}"
                            )));
                        }
                    }
                }
                tokens.push(Token::Value(value));
            }
            c if is_ident_start(c) => {
                let mut name = String::new();
                while let Some(&(_, ch)) = chars.peek() {
                    if is_ident_continue(ch) {
                        name.push(ch);
                        chars.next();
                    } else {
                        break;
                    }
                }
                tokens.push(Token::Ident(name));
            }
            _ => {
                return Err(ParseError::new(format!(
                    "unexpected character `{c}` at position {i}"
                )));
            }
        }
    }

    Ok(tokens)
}

fn is_ident_start(c: char) -> bool {
    c.is_ascii_alphabetic()
}

fn is_ident_continue(c: char) -> bool {
    c.is_ascii_alphanumeric()
}

struct Parser {
    tokens: Vec<Token>,
    pos: usize,
}

impl Parser {
    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.pos)
    }

    fn advance(&mut self) -> Option<Token> {
        let token = self.tokens.get(self.pos).cloned();
        if token.is_some() {
            self.pos += 1;
        }
        token
    }

    fn expect(&mut self, expected: &Token) -> Result<(), ParseError> {
        match self.advance() {
            Some(ref token) if token == expected => Ok(()),
            Some(token) => Err(ParseError::new(format!(
                "expected {expected:?}, found {token:?}"
            ))),
            None => Err(ParseError::new(format!(
                "expected {expected:?}, found end of input"
            ))),
        }
    }

    fn parse_or(&mut self) -> Result<Rule, ParseError> {
        let mut left = self.parse_and()?;
        while matches!(self.peek(), Some(Token::Or)) {
            self.advance();
            let right = self.parse_and()?;
            left = Rule::Or(Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    fn parse_and(&mut self) -> Result<Rule, ParseError> {
        let mut left = self.parse_unary()?;
        while matches!(self.peek(), Some(Token::And)) {
            self.advance();
            let right = self.parse_unary()?;
            left = Rule::And(Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    fn parse_unary(&mut self) -> Result<Rule, ParseError> {
        if matches!(self.peek(), Some(Token::Not)) {
            self.advance();
            let inner = self.parse_unary()?;
            Ok(Rule::Not(Box::new(inner)))
        } else {
            self.parse_primary()
        }
    }

    fn parse_primary(&mut self) -> Result<Rule, ParseError> {
        match self.advance() {
            Some(Token::LParen) => {
                let rule = self.parse_or()?;
                self.expect(&Token::RParen)?;
                Ok(rule)
            }
            Some(Token::Ident(name)) => {
                self.expect(&Token::LParen)?;
                let args = self.parse_args()?;
                Ok(Rule::Matcher(Matcher { name, args }))
            }
            Some(token) => Err(ParseError::new(format!(
                "expected a matcher or `(`, found {token:?}"
            ))),
            None => Err(ParseError::new(
                "expected a matcher or `(`, found end of input",
            )),
        }
    }

    fn parse_args(&mut self) -> Result<Vec<String>, ParseError> {
        let mut args = Vec::new();

        if matches!(self.peek(), Some(Token::RParen)) {
            self.advance();
            return Ok(args);
        }

        loop {
            match self.advance() {
                Some(Token::Value(value)) => args.push(value),
                Some(token) => {
                    return Err(ParseError::new(format!(
                        "expected a value, found {token:?}"
                    )));
                }
                None => {
                    return Err(ParseError::new("expected a value, found end of input"));
                }
            }

            match self.advance() {
                Some(Token::Comma) => continue,
                Some(Token::RParen) => break,
                Some(token) => {
                    return Err(ParseError::new(format!(
                        "expected `,` or `)`, found {token:?}"
                    )));
                }
                None => {
                    return Err(ParseError::new("expected `,` or `)`, found end of input"));
                }
            }
        }

        Ok(args)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn matcher(name: &str, args: &[&str]) -> Rule {
        Rule::Matcher(Matcher {
            name: name.to_string(),
            args: args.iter().map(|a| a.to_string()).collect(),
        })
    }

    #[test]
    fn single_matcher() {
        let rule = Rule::parse("Host(`example.com`)").unwrap();
        assert_eq!(rule, matcher("Host", &["example.com"]));
    }

    #[test]
    fn multiple_arguments() {
        let rule = Rule::parse("Header(`X-Foo`, `bar`)").unwrap();
        assert_eq!(rule, matcher("Header", &["X-Foo", "bar"]));
    }

    #[test]
    fn escaped_double_quote_values() {
        let rule = Rule::parse(r#"Host("example.com")"#).unwrap();
        assert_eq!(rule, matcher("Host", &["example.com"]));

        let rule = Rule::parse(r#"Header("X", "a\"b")"#).unwrap();
        assert_eq!(rule, matcher("Header", &["X", "a\"b"]));
    }

    #[test]
    fn negation() {
        let rule = Rule::parse("!Path(`/foo`)").unwrap();
        assert_eq!(rule, Rule::Not(Box::new(matcher("Path", &["/foo"]))));
    }

    #[test]
    fn and_or_precedence() {
        // && binds tighter than ||, so this parses as a || (b && c).
        let rule = Rule::parse("Host(`a`) || Host(`b`) && Path(`/c`)").unwrap();
        assert_eq!(
            rule,
            Rule::Or(
                Box::new(matcher("Host", &["a"])),
                Box::new(Rule::And(
                    Box::new(matcher("Host", &["b"])),
                    Box::new(matcher("Path", &["/c"])),
                )),
            )
        );
    }

    #[test]
    fn parentheses_override_precedence() {
        let rule = Rule::parse("(Host(`a`) || Host(`b`)) && Path(`/c`)").unwrap();
        assert_eq!(
            rule,
            Rule::And(
                Box::new(Rule::Or(
                    Box::new(matcher("Host", &["a"])),
                    Box::new(matcher("Host", &["b"])),
                )),
                Box::new(matcher("Path", &["/c"])),
            )
        );
    }

    #[test]
    fn and_is_left_associative() {
        let rule = Rule::parse("Host(`a`) && Host(`b`) && Host(`c`)").unwrap();
        assert_eq!(
            rule,
            Rule::And(
                Box::new(Rule::And(
                    Box::new(matcher("Host", &["a"])),
                    Box::new(matcher("Host", &["b"])),
                )),
                Box::new(matcher("Host", &["c"])),
            )
        );
    }

    #[test]
    fn negated_group() {
        let rule = Rule::parse("!(Host(`a`) || Host(`b`))").unwrap();
        assert_eq!(
            rule,
            Rule::Not(Box::new(Rule::Or(
                Box::new(matcher("Host", &["a"])),
                Box::new(matcher("Host", &["b"])),
            )))
        );
    }

    #[test]
    fn display_round_trips() {
        for input in [
            "Host(`a`) || Host(`b`) && Path(`/c`)",
            "(Host(`a`) || Host(`b`)) && Path(`/c`)",
            "!Path(`/foo`)",
            "!(Host(`a`) || Host(`b`))",
            "Header(`X-Foo`, `bar`)",
        ] {
            let rule = Rule::parse(input).unwrap();
            let rendered = rule.to_string();
            let reparsed = Rule::parse(&rendered).unwrap();
            assert_eq!(
                rule, reparsed,
                "round trip failed for {input:?} -> {rendered:?}"
            );
        }
    }

    #[test]
    fn errors_on_garbage() {
        assert!(Rule::parse("Host(`a`) &&").is_err());
        assert!(Rule::parse("Host(`a`").is_err());
        assert!(Rule::parse("Host`a`)").is_err());
        assert!(Rule::parse("Host(`a`) Host(`b`)").is_err());
        assert!(Rule::parse("").is_err());
        assert!(Rule::parse("Host('a')").is_err()); // single quotes not allowed
    }
}
