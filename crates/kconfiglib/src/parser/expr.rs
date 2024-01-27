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
    Cmp(ExprCmpOp, Box<LocExpr>, Box<LocExpr>),

    /// Unary negation.
    Not(Box<LocExpr>),

    /// Boolean AND.
    And(Box<LocExpr>, Box<LocExpr>),

    /// Boolean OR.
    Or(Box<LocExpr>, Box<LocExpr>),
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

/// An expression with location information.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LocExpr {
    /// The expression.
    pub expr: Expr,

    /// The location of the expression.
    pub location: Location,
}

impl LocExpr {
    /// Create a new located expression from the given raw expression and location.
    pub fn new(expr: Expr, location: Location) -> Self {
        Self {
            expr,
            location,
        }
    }

    /// Parse an expression.
    pub fn parse(prev: Location, tokens: &mut TokenLine) -> Result<Self, KConfigError> {
        let result = Self::parse_top(prev, tokens)?;

        if let Some(t) = tokens.peek() {
            if t.token != Token::If {
                let loc = t.location();
                return Err(KConfigError::unexpected(&t.token, Expected::Eol, loc));
            }
        }

        Ok(result)
    }

    /// Parse a `depends on <expr>` line.
    pub fn parse_depends_on(tokens: &mut TokenLine) -> Result<Self, KConfigError> {
        Self::parse_dep_vis(tokens, "depends", Token::On, Expected::On)
    }

    /// Parse a `visible if <expr>` line.
    pub fn parse_visible_if(tokens: &mut TokenLine) -> Result<Self, KConfigError> {
        Self::parse_dep_vis(tokens, "visible", Token::If, Expected::If)
    }

    /// The guts of the parsing logic for `depends on <expr>` or `visible if <expr>` lines.
    fn parse_dep_vis(
        tokens: &mut TokenLine,
        statement: &str,
        preposition: Token,
        expected: Expected,
    ) -> Result<Self, KConfigError> {
        let Some(cmd) = tokens.next() else {
            panic!("Expected {statement} command");
        };

        // prep_token ("preposition token") is either `if` or `on`.
        let Some(prep_token) = tokens.next() else {
            return Err(KConfigError::missing(expected, cmd.location()));
        };

        if prep_token.token != preposition {
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
    fn parse_top(prev: Location, tokens: &mut TokenLine) -> Result<Self, KConfigError> {
        Self::parse_or(prev, tokens)
    }

    /// Parse an OR (`||`) expression, or return the underlying AND expression.
    fn parse_or(prev: Location, tokens: &mut TokenLine) -> Result<Self, KConfigError> {
        let lhs = Self::parse_and(prev, tokens)?;
        let Some(op) = tokens.peek() else {
            return Ok(lhs);
        };

        if op.token != Token::Or {
            return Ok(lhs);
        }

        let op = tokens.next().unwrap();
        let loc = lhs.location();
        let rhs = Self::parse_top(op.location(), tokens)?;
        Ok(Self::new(Expr::Or(lhs.into(), rhs.into()), loc))
    }

    /// Parse an AND ('&&') expression, or return the underlying comparison expression.
    fn parse_and(prev: Location, tokens: &mut TokenLine) -> Result<Self, KConfigError> {
        let lhs = Self::parse_comparison(prev, tokens)?;
        let Some(op) = tokens.peek() else {
            return Ok(lhs);
        };

        if op.token != Token::And {
            return Ok(lhs);
        }

        let op = tokens.next().unwrap();
        let loc = lhs.location();
        let rhs = Self::parse_top(op.location(), tokens)?;
        Ok(Self::new(Expr::And(lhs.into(), rhs.into()), loc))
    }

    /// Parse a comparison expression, or return the underlying unary-not expression.
    fn parse_comparison(prev: Location, tokens: &mut TokenLine) -> Result<Self, KConfigError> {
        let lhs = Self::parse_unary_not(prev, tokens)?;

        let Some(op) = tokens.peek() else {
            return Ok(lhs);
        };

        if !op.token.is_cmp() {
            return Ok(lhs);
        }

        let op = op.clone();

        _ = tokens.next();
        let rhs = Self::parse_top(op.location(), tokens)?;
        let loc = lhs.location();
        let cmp = op.token.try_into().unwrap();

        Ok(Self::new(Expr::Cmp(cmp, lhs.into(), rhs.into()), loc))
    }

    /// Parse a unary not expression, or return the underlying terminal expression.
    fn parse_unary_not(prev: Location, tokens: &mut TokenLine) -> Result<Self, KConfigError> {
        let Some(token) = tokens.peek() else {
            return Err(KConfigError::missing(Expected::Expr, prev));
        };

        if token.token == Token::Not {
            let loc = token.location();
            _ = tokens.next();
            let expr = Self::parse_top(prev, tokens)?;
            Ok(Self::new(Expr::Not(expr.into()), loc))
        } else {
            Self::parse_terminal(prev, tokens)
        }
    }

    /// Parse a terminal or an expression in parentheses.
    fn parse_terminal(prev: Location, tokens: &mut TokenLine) -> Result<Self, KConfigError> {
        let Some(token) = tokens.peek() else {
            return Err(KConfigError::missing(Expected::Expr, prev));
        };

        let loc = token.location();
        let expr = match &token.token {
            Token::Symbol(s) => Expr::Symbol(s.clone()),
            Token::HexLit(i) => Expr::Hex(*i),
            Token::IntLit(i) => Expr::Integer(*i),
            Token::StrLit(s) => Expr::String(s.clone()),
            Token::LParen => return Self::parse_paren(prev, tokens),
            _ => return Err(KConfigError::unexpected(token, Expected::Expr, token.location())),
        };

        _ = tokens.next();
        Ok(Self::new(expr, loc))
    }

    /// Parse an expression in parentheses.
    fn parse_paren(prev: Location, tokens: &mut TokenLine) -> Result<Self, KConfigError> {
        trace!("parse_paren: tokens={tokens:?}");

        let Some(lparen) = tokens.next() else {
            return Err(KConfigError::missing(Expected::Expr, prev));
        };

        if lparen.token != Token::LParen {
            return Err(KConfigError::unexpected(&lparen.token, Expected::Expr, lparen.location()));
        }

        let result = Self::parse_top(lparen.location(), tokens)?;

        let Some(rparen) = tokens.next() else {
            return Err(KConfigError::missing(Expected::RParen, lparen.location()));
        };

        if rparen.token != Token::RParen {
            return Err(KConfigError::unexpected(&rparen.token, Expected::RParen, rparen.location()));
        }

        Ok(result)
    }
}

impl Display for Expr {
    fn fmt(&self, f: &mut Formatter) -> FmtResult {
        match self {
            Self::Symbol(s) => write!(f, "{s}"),
            Self::Hex(i) => write!(f, "0x{i:x}"),
            Self::Integer(i) => write!(f, "{i}"),
            Self::String(s) => write!(f, "{s:?}"),
            Self::Cmp(op, lhs, rhs) => {
                let lhs = match lhs.expr {
                    Self::And(_, _) | Self::Or(_, _) => format!("({})", lhs.expr),
                    _ => format!("{}", lhs.expr),
                };

                let rhs = match rhs.expr {
                    Self::And(_, _) | Self::Or(_, _) => format!("({})", rhs.expr),
                    _ => format!("{}", rhs.expr),
                };

                write!(f, "{lhs} {op} {rhs}")
            }
            Self::Not(inner) => match inner.expr {
                Self::Cmp(_, _, _) | Self::And(_, _) | Self::Or(_, _) => write!(f, "!({})", inner.expr),
                _ => write!(f, "!{}", inner.expr),
            },
            Self::And(lhs, rhs) => {
                let lhs = match lhs.expr {
                    Self::Or(_, _) => format!("({})", lhs.expr),
                    _ => format!("{}", lhs.expr),
                };

                let rhs = match rhs.expr {
                    Self::Or(_, _) => format!("({})", rhs.expr),
                    _ => format!("{}", rhs.expr),
                };

                write!(f, "{lhs} && {rhs}")
            }
            Self::Or(lhs, rhs) => write!(f, "{} || {}", lhs.expr, rhs.expr),
        }
    }
}

impl Display for ExprCmpOp {
    fn fmt(&self, f: &mut Formatter) -> FmtResult {
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

impl TryFrom<Token> for ExprCmpOp {
    type Error = ();

    fn try_from(token: Token) -> Result<Self, Self::Error> {
        match token {
            Token::Eq => Ok(Self::Eq),
            Token::Ne => Ok(Self::Ne),
            Token::Lt => Ok(Self::Lt),
            Token::Le => Ok(Self::Le),
            Token::Gt => Ok(Self::Gt),
            Token::Ge => Ok(Self::Ge),
            _ => Err(()),
        }
    }
}

impl Located for LocExpr {
    fn location(&self) -> Location {
        self.location
    }
}

#[cfg(test)]
mod tests {
    use {
        crate::parser::{LocToken, Location, Token},
        std::path::Path,
    };

    #[test_log::test]
    fn two_or_comparison() {
        let path = Path::new("test");
        let tokens = vec![
            LocToken::new(Token::Symbol("FOO".to_string()), Location::new(path, 1, 1)),
            LocToken::new(Token::Eq, Location::new(path, 1, 5)),
            LocToken::new(Token::Symbol("BAR".to_string()), Location::new(path, 1, 7)),
            LocToken::new(Token::Or, Location::new(path, 1, 11)),
            LocToken::new(Token::Symbol("BAZ".to_string()), Location::new(path, 1, 13)),
            LocToken::new(Token::Eq, Location::new(path, 1, 17)),
            LocToken::new(Token::Symbol("QUX".to_string()), Location::new(path, 1, 19)),
        ];

        let mut token_line = crate::parser::TokenLine::new(&tokens);
        let _expr = super::LocExpr::parse(Location::new(path, 1, 1), &mut token_line).unwrap();
    }
}
