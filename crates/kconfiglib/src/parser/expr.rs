use {
    crate::{
        parser::{Expected, Located, Location, Token, TokenLine, Tristate},
        KConfigError,
    },
    log::trace,
    std::{
        collections::HashSet,
        fmt::{Display, Formatter, Result as FmtResult},
    },
};

/// An expression in the KConfig language.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Expr {
    /// Tristate (terminal).
    Tristate(Tristate),

    /// Named symbol (terminal).
    Symbol(String),

    /// Hex constant (terminal).
    Hex(u64),

    /// Integer constant (terminal).
    Int(i64),

    /// String literal (terminal).
    String(String),

    /// Comparison expression.
    Cmp(ExprCmpOp, Box<Expr>, Box<Expr>),

    /// Unary negation.
    Not(Box<Expr>),

    /// Boolean AND.
    And(Box<Expr>, Box<Expr>),

    /// Boolean OR.
    Or(Box<Expr>, Box<Expr>),
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
    /// Create an expression that is a symbol with the given name.
    /// This handles `y`, `n`, and `m` symbols as tristate values.
    pub fn symbol(name: &str) -> Self {
        match name {
            "y" => Self::Tristate(Tristate::True),
            "n" => Self::Tristate(Tristate::False),
            "m" => Self::Tristate(Tristate::Maybe),
            _ => Self::Symbol(name.to_string()),
        }
    }

    /// Create a expression that is the logical and between two expressions.
    pub fn and(lhs: Expr, rhs: Expr) -> Self {
        match lhs {
            Self::Tristate(Tristate::True) => rhs,
            Self::Tristate(Tristate::False) => Self::Tristate(Tristate::False),
            _ => match rhs {
                Self::Tristate(Tristate::True) => lhs,
                Self::Tristate(Tristate::False) => Self::Tristate(Tristate::False),
                _ => Self::And(lhs.into(), rhs.into()),
            },
        }
    }

    /// Create a expression that is the logical or between two expressions.
    pub fn or(lhs: Expr, rhs: Expr) -> Self {
        match lhs {
            Self::Tristate(Tristate::True) => Self::Tristate(Tristate::True),
            Self::Tristate(Tristate::False) => rhs,
            _ => match rhs {
                Self::Tristate(Tristate::True) => Self::Tristate(Tristate::True),
                Self::Tristate(Tristate::False) => lhs,
                _ => Self::Or(lhs.into(), rhs.into()),
            },
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
        let rhs = Self::parse_top(op.location(), tokens)?;
        Ok(Self::Or(Box::new(lhs), Box::new(rhs)))
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
        let rhs = Self::parse_top(op.location(), tokens)?;
        Ok(Self::And(lhs.into(), rhs.into()))
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
        let cmp = op.token.try_into().unwrap();

        Ok(Self::Cmp(cmp, lhs.into(), rhs.into()))
    }

    /// Parse a unary not expression, or return the underlying terminal expression.
    fn parse_unary_not(prev: Location, tokens: &mut TokenLine) -> Result<Self, KConfigError> {
        let Some(token) = tokens.peek() else {
            return Err(KConfigError::missing(Expected::Expr, prev));
        };

        if token.token == Token::Not {
            _ = tokens.next();
            let expr = Self::parse_top(prev, tokens)?;
            Ok(Self::Not(expr.into()))
        } else {
            Self::parse_terminal(prev, tokens)
        }
    }

    /// Parse a terminal or an expression in parentheses.
    fn parse_terminal(prev: Location, tokens: &mut TokenLine) -> Result<Self, KConfigError> {
        let Some(token) = tokens.peek() else {
            return Err(KConfigError::missing(Expected::Expr, prev));
        };

        let expr = match &token.token {
            Token::Symbol(s) => Expr::symbol(s),
            Token::HexLit(i) => Expr::Hex(*i),
            Token::IntLit(i) => Expr::Int(*i),
            Token::StrLit(s) => Expr::String(s.clone()),
            Token::LParen => return Self::parse_paren(prev, tokens),
            _ => return Err(KConfigError::unexpected(token, Expected::Expr, token.location())),
        };

        _ = tokens.next();
        Ok(expr)
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

    /// Returns all of the symbols found in this expression.
    pub fn symbols(&self) -> HashSet<String> {
        let mut result = HashSet::new();
        self.symbols_into(&mut result);
        result
    }

    /// Inserts all of the symbols found in this expression into the given set.
    pub(crate) fn symbols_into(&self, result: &mut HashSet<String>) {
        match self {
            Self::Symbol(s) => {
                result.insert(s.clone());
            }
            Self::Cmp(_, lhs, rhs) => {
                lhs.symbols_into(result);
                rhs.symbols_into(result);
            }
            Self::Not(inner) => {
                inner.symbols_into(result);
            }
            Self::And(lhs, rhs) => {
                lhs.symbols_into(result);
                rhs.symbols_into(result);
            }
            Self::Or(lhs, rhs) => {
                lhs.symbols_into(result);
                rhs.symbols_into(result);
            }
            _ => (),
        }
    }
}

impl Display for Expr {
    fn fmt(&self, f: &mut Formatter) -> FmtResult {
        match self {
            Self::Tristate(t) => Display::fmt(t, f),
            Self::Symbol(s) => write!(f, "{s}"),
            Self::Hex(i) => write!(f, "0x{i:x}"),
            Self::Int(i) => write!(f, "{i}"),
            Self::String(s) => write!(f, "{s:?}"),
            Self::Cmp(op, lhs, rhs) => {
                let lhs = match &**lhs {
                    Self::And(_, _) | Self::Or(_, _) => format!("({lhs})"),
                    _ => lhs.to_string(),
                };

                let rhs = match &**rhs {
                    Self::And(_, _) | Self::Or(_, _) => format!("({rhs})"),
                    _ => rhs.to_string(),
                };

                write!(f, "{lhs} {op} {rhs}")
            }
            Self::Not(inner) => match &**inner {
                Self::Cmp(_, _, _) | Self::And(_, _) | Self::Or(_, _) => write!(f, "!({inner})"),
                _ => write!(f, "!{inner}"),
            },
            Self::And(lhs, rhs) => {
                let lhs = match **lhs {
                    Self::Or(_, _) => format!("({lhs})"),
                    _ => lhs.to_string(),
                };

                let rhs = match &**rhs {
                    Self::Or(_, _) => format!("({rhs})"),
                    _ => rhs.to_string(),
                };

                write!(f, "{lhs} && {rhs}")
            }
            Self::Or(lhs, rhs) => write!(f, "{lhs} || {rhs}"),
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

#[cfg(test)]
mod tests {
    use {
        crate::parser::{Expr, LocToken, Location, Token},
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
        let _expr = Expr::parse(Location::new(path, 1, 1), &mut token_line).unwrap();
    }
}
