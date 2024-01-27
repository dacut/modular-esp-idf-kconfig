use {
    crate::parser::{Expected, KConfigError, Located, Location, Token, TokenLine},
    log::trace,
    std::fmt::{Display, Formatter, Result as FmtResult},
};

/// An expression in the KConfig language.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Expr {
    /// Named symbol (terminal).
    Symbol(String),

    /// Hex constant (terminal).
    Hex(u64),

    /// Integer constant (terminal).
    Integer(i64),

    /// String literal (terminal).
    String(String),

    /// Comparison expression.
    Cmp(ExprCmpOp, Located<Box<Expr>>, Located<Box<Expr>>),

    /// Unary negation.
    Not(Located<Box<Expr>>),

    /// Boolean AND.
    And(Located<Box<Expr>>, Located<Box<Expr>>),

    /// Boolean OR.
    Or(Located<Box<Expr>>, Located<Box<Expr>>),
}

/// Comparison operator
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExprCmpOp {
    /// Equals
    Eq,

    /// Not equals
    Ne,

    /// Less than
    Lt,

    /// Less than or equal
    Le,

    /// Greater than
    Gt,

    /// Greater than or equal
    Ge,
}

impl Expr {
    /// Parse an expression.
    pub fn parse(prev: &Location, tokens: &mut TokenLine) -> Result<Located<Self>, KConfigError> {
        let result = Self::parse_top(prev, tokens)?;

        if let Some(t) = tokens.peek() {
            if !matches!(*t.as_ref(), Token::If) {
                let loc = t.location().clone();
                return Err(KConfigError::unexpected(t.as_ref(), Expected::Eol, &loc));
            }
        }

        Ok(result)
    }

    /// Parse a `depends on <expr>` line.
    pub fn parse_depends_on(tokens: &mut TokenLine) -> Result<Located<Self>, KConfigError> {
        Self::parse_dep_vis(tokens, "depends", Token::On, Expected::On)
    }

    /// Parse a `visible if <expr>` line.
    pub fn parse_visible_if(tokens: &mut TokenLine) -> Result<Located<Self>, KConfigError> {
        Self::parse_dep_vis(tokens, "visible", Token::If, Expected::If)
    }

    /// The guts of the parsing logic for `depends on <expr>` or `visible if <expr>` lines.
    fn parse_dep_vis(
        tokens: &mut TokenLine,
        statement: &str,
        preposition: Token,
        expected: Expected,
    ) -> Result<Located<Self>, KConfigError> {
        let Some(cmd) = tokens.next() else {
            panic!("Expected {statement} command");
        };

        // prep_token ("preposition token") is either `if` or `on`.
        let Some(prep_token) = tokens.next() else {
            return Err(KConfigError::missing(expected, cmd.location()));
        };

        if prep_token.as_ref() != &preposition {
            return Err(KConfigError::unexpected(prep_token, expected, prep_token.location()));
        }

        let expr = Self::parse(prep_token.location(), tokens)?;

        if let Some(unexpected) = tokens.next() {
            return Err(KConfigError::unexpected(unexpected, Expected::Eol, unexpected.location()));
        }

        Ok(expr)
    }

    /// Parse the expression from a peekable token iterator.
    #[inline(always)]
    fn parse_top(prev: &Location, tokens: &mut TokenLine) -> Result<Located<Self>, KConfigError> {
        trace!("parse_top: tokens={tokens:?}");
        Self::parse_or(prev, tokens)
    }

    /// Parse an OR (`||`) expression, or return the underlying AND expression.
    fn parse_or(prev: &Location, tokens: &mut TokenLine) -> Result<Located<Self>, KConfigError> {
        trace!("parse_or: tokens={tokens:?}");
        let lhs = Self::parse_and(prev, tokens)?;
        let Some(op) = tokens.peek() else {
            return Ok(lhs);
        };

        if !matches!(op.as_ref(), Token::Or) {
            return Ok(lhs);
        }

        let op = tokens.next().unwrap();
        let loc = lhs.location().clone();
        let rhs = Self::parse_top(op.location(), tokens)?;
        Ok(Located::new(Expr::Or(lhs.into(), rhs.into()), loc))
    }

    /// Parse an AND ('&&') expression, or return the underlying comparison expression.
    fn parse_and(prev: &Location, tokens: &mut TokenLine) -> Result<Located<Self>, KConfigError> {
        trace!("parse_and: tokens={tokens:?}");
        let lhs = Self::parse_comparison(prev, tokens)?;
        let Some(op) = tokens.peek() else {
            return Ok(lhs);
        };

        if !matches!(op.as_ref(), Token::And) {
            return Ok(lhs);
        }

        let op = tokens.next().unwrap();
        let loc = lhs.location().clone();
        let rhs = Self::parse_top(op.location(), tokens)?;
        Ok(Located::new(Expr::And(lhs.into(), rhs.into()), loc))
    }

    /// Parse a comparison expression, or return the underlying unary-not expression.
    fn parse_comparison(prev: &Location, tokens: &mut TokenLine) -> Result<Located<Self>, KConfigError> {
        trace!("parse_comparison: tokens={tokens:?}");
        let lhs = Self::parse_unary_not(prev, tokens)?;

        let Some(op) = tokens.peek() else {
            return Ok(lhs);
        };

        if !matches!(op.as_ref(), Token::Eq | Token::Ne | Token::Lt | Token::Le | Token::Gt | Token::Ge) {
            return Ok(lhs);
        }

        let op = op.clone();

        _ = tokens.next();
        let rhs = Self::parse_top(op.location(), tokens)?;
        let loc = lhs.location().clone();

        let cmp = match op.as_ref() {
            Token::Eq => ExprCmpOp::Eq,
            Token::Ne => ExprCmpOp::Ne,
            Token::Lt => ExprCmpOp::Lt,
            Token::Le => ExprCmpOp::Le,
            Token::Gt => ExprCmpOp::Gt,
            Token::Ge => ExprCmpOp::Ge,
            _ => unreachable!(),
        };

        Ok(Located::new(Expr::Cmp(cmp, lhs.into(), rhs.into()), loc))
    }

    /// Parse a unary not expression, or return the underlying terminal expression.
    fn parse_unary_not(prev: &Location, tokens: &mut TokenLine) -> Result<Located<Self>, KConfigError> {
        trace!("parse_unary_not: tokens={tokens:?}");

        let Some(token) = tokens.peek() else {
            return Err(KConfigError::missing(Expected::Expr, prev));
        };

        match token.as_ref() {
            Token::Not => {
                let loc = token.location().clone();
                _ = tokens.next();
                let expr = Self::parse_top(prev, tokens)?;
                Ok(Located::new(Expr::Not(expr.into()), loc))
            }
            _ => Ok(Self::parse_terminal(prev, tokens)?),
        }
    }

    /// Parse a terminal or an expression in parentheses.
    fn parse_terminal(prev: &Location, tokens: &mut TokenLine) -> Result<Located<Self>, KConfigError> {
        trace!("parse_terminal: tokens={tokens:?}");

        let Some(token) = tokens.peek() else {
            return Err(KConfigError::missing(Expected::Expr, prev));
        };

        let loc = token.location().clone();

        let expr = match token.as_ref() {
            Token::Symbol(s) => Expr::Symbol(s.clone()),
            Token::HexLit(i) => Expr::Hex(*i),
            Token::IntLit(i) => Expr::Integer(*i),
            Token::StrLit(s) => Expr::String(s.clone()),
            Token::LParen => return Self::parse_paren(prev, tokens),
            _ => return Err(KConfigError::unexpected(token.as_ref(), Expected::Expr, token.location())),
        };

        _ = tokens.next();
        Ok(Located::new(expr, loc))
    }

    /// Parse an expression in parentheses.
    fn parse_paren(prev: &Location, tokens: &mut TokenLine) -> Result<Located<Self>, KConfigError> {
        trace!("parse_paren: tokens={tokens:?}");

        let Some(lparen) = tokens.next() else {
            return Err(KConfigError::missing(Expected::Expr, prev));
        };

        if !matches!(lparen.as_ref(), Token::LParen) {
            return Err(KConfigError::unexpected(lparen.as_ref(), Expected::Expr, lparen.location()));
        }

        let result = Self::parse_top(lparen.location(), tokens)?;

        let Some(rparen) = tokens.next() else {
            return Err(KConfigError::missing(Expected::RParen, lparen.location()));
        };

        if !matches!(rparen.as_ref(), Token::RParen) {
            return Err(KConfigError::unexpected(rparen.as_ref(), Expected::RParen, rparen.location()));
        }

        Ok(result)
    }
}

impl Display for Expr {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        match self {
            Self::Symbol(s) => write!(f, "{s}"),
            Self::Hex(i) => write!(f, "0x{i:x}"),
            Self::Integer(i) => write!(f, "{i}"),
            Self::String(s) => write!(f, "{s:?}"),
            Self::Cmp(op, lhs, rhs) => {
                let lhs = lhs.as_ref();
                let rhs = rhs.as_ref();

                let lhs = match lhs.as_ref() {
                    Self::And(_, _) | Self::Or(_, _) => format!("({})", lhs),
                    _ => format!("{}", lhs),
                };

                let rhs = match rhs.as_ref() {
                    Self::And(_, _) | Self::Or(_, _) => format!("({})", rhs),
                    _ => format!("{}", rhs),
                };

                write!(f, "{lhs} {op} {rhs}")
            }
            Self::Not(expr) => {
                let expr = expr.as_ref();
                let expr = match expr.as_ref() {
                    Self::Cmp(_, _, _) | Self::And(_, _) | Self::Or(_, _) => format!("({})", expr),
                    _ => format!("{}", expr),
                };
                write!(f, "!{expr}")
            }
            Self::And(lhs, rhs) => {
                let lhs = lhs.as_ref();
                let rhs = rhs.as_ref();

                let lhs = match lhs.as_ref() {
                    Self::Or(_, _) => format!("({})", lhs),
                    _ => format!("{}", lhs),
                };

                let rhs = match rhs.as_ref() {
                    Self::Or(_, _) => format!("({})", rhs),
                    _ => format!("{}", rhs),
                };

                write!(f, "{lhs} && {rhs}")
            }
            Self::Or(lhs, rhs) => write!(f, "{} || {}", lhs, rhs),
        }
    }
}

impl Display for ExprCmpOp {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        match self {
            Self::Eq => write!(f, "=="),
            Self::Ne => write!(f, "!="),
            Self::Lt => write!(f, "<"),
            Self::Le => write!(f, "<="),
            Self::Gt => write!(f, ">"),
            Self::Ge => write!(f, ">="),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::parser::{Located, Location, Token};

    #[test_log::test]
    fn two_or_comparison() {
        let tokens = vec![
            Located::new(Token::Symbol("FOO".to_string()), Location::new("test", 1, 1)),
            Located::new(Token::Eq, Location::new("test", 1, 5)),
            Located::new(Token::Symbol("BAR".to_string()), Location::new("test", 1, 7)),
            Located::new(Token::Or, Location::new("test", 1, 11)),
            Located::new(Token::Symbol("BAZ".to_string()), Location::new("test", 1, 13)),
            Located::new(Token::Eq, Location::new("test", 1, 17)),
            Located::new(Token::Symbol("QUX".to_string()), Location::new("test", 1, 19)),
        ];

        let mut token_line = crate::parser::TokenLine::new(&tokens);
        let _expr = super::Expr::parse(&Location::new("test", 1, 1), &mut token_line).unwrap();
    }
}
