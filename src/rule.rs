//! Parser for the Traefik HTTP router match rule syntax.
//!
//! Grammar (in order of increasing precedence):
//!
//! ```text
//! expr    := or
//! or      := and ( "||" and )*
//! and     := unary ( "&&" unary )*
//! unary   := "!" unary | primary
//! primary := "(" expr ")" | matcher
//! matcher := IDENT "(" ( value ( "," value )* )? ")"
//! value   := "`" ... "`" | "\"" ... "\""
//! ```
//!
//! See <https://doc.traefik.io/traefik/reference/routing-configuration/http/routing/rules-and-priority/#rules>.

use std::fmt::{self, Display};

/// The parsed abstract syntax tree of a Traefik match rule.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Rule {
    /// A leaf matcher such as `Host(`example.com`)`.
    Matcher(Matcher),
    /// Negation, e.g. `!Path(`/foo`)`.
    Not(Box<Rule>),
    /// Logical AND of two rules.
    And(Box<Rule>, Box<Rule>),
    /// Logical OR of two rules.
    Or(Box<Rule>, Box<Rule>),
}

/// A single matcher function call, e.g. `Header(`X-Foo`, `bar`)`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Matcher {
    pub name: String,
    pub args: Vec<String>,
}

impl Rule {
    fn precedence(&self) -> u8 {
        match self {
            Rule::Or(..) => 1,
            Rule::And(..) => 2,
            Rule::Not(..) => 3,
            Rule::Matcher(..) => 4,
        }
    }

    /// Render the rule, parenthesizing children whose precedence is lower
    /// than the surrounding context.
    fn fmt_prec(&self, f: &mut fmt::Formatter<'_>, parent: u8) -> fmt::Result {
        let needs_parens = self.precedence() < parent;
        if needs_parens {
            write!(f, "(")?;
        }
        match self {
            Rule::Matcher(m) => {
                write!(f, "{}(", m.name)?;
                for (i, arg) in m.args.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "`{arg}`")?;
                }
                write!(f, ")")?;
            }
            Rule::Not(inner) => {
                write!(f, "!")?;
                inner.fmt_prec(f, 3)?;
            }
            Rule::And(lhs, rhs) => {
                lhs.fmt_prec(f, 2)?;
                write!(f, " && ")?;
                rhs.fmt_prec(f, 2)?;
            }
            Rule::Or(lhs, rhs) => {
                lhs.fmt_prec(f, 1)?;
                write!(f, " || ")?;
                rhs.fmt_prec(f, 1)?;
            }
        }
        if needs_parens {
            write!(f, ")")?;
        }
        Ok(())
    }
}

impl Display for Rule {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.fmt_prec(f, 0)
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
