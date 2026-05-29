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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Matcher {
    Host { host: String },
    Path { path: String },
    PathPrefix { path: String },
    PathRegexp { pattern: String },
}

impl Display for Matcher {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Matcher::Host { host } => write!(f, "Host(`{host}`)"),
            Matcher::Path { path } => write!(f, "Path(`{path}`)"),
            Matcher::PathPrefix { path } => write!(f, "PathPrefix(`{path}`)"),
            Matcher::PathRegexp { pattern } => write!(f, "PathRegexp(`{pattern}`)"),
        }
    }
}

impl Matcher {
    fn match_request(&self, request: &Request) -> bool {
        let url = &request.url;
        match self {
            Matcher::Host { host } => url
                .host()
                .map(|h| host == &h.to_string())
                .unwrap_or_default(),
            Matcher::Path { path } => url.path() == path,
            Matcher::PathPrefix { path } => url.path().starts_with(path),
            Matcher::PathRegexp { pattern } => {
                let re = regex::Regex::new(&pattern).expect("malformed regex");
                re.is_match(url.path())
            }
        }
    }
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
            Rule::Matcher(m) => write!(f, "{}", m)?,
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

    pub fn match_request(&self, request: &Request) -> bool {
        match self {
            Rule::Matcher(matcher) => matcher.match_request(request),
            Rule::Not(rule) => !rule.match_request(request),
            Rule::And(r1, r2) => r1.match_request(request) && r2.match_request(request),
            Rule::Or(r1, r2) => r1.match_request(request) || r2.match_request(request),
        }
    }
}

impl Display for Rule {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.fmt_prec(f, 0)
    }
}

pub struct Request {
    pub url: url::Url,
}
