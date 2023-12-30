use {
    crate::parser::{hws0, KConfigError},
    nom::{
        branch::alt,
        bytes::complete::tag,
        character::complete::{alpha1, alphanumeric1, char, digit1, hex_digit1, oct_digit1, one_of},
        combinator::{map, map_res, opt, recognize},
        error::{FromExternalError, ParseError},
        multi::{many0, many0_count, many1},
        sequence::{delimited, pair, preceded, tuple},
        IResult,
    },
    std::num::ParseIntError,
};

/// An expression in the KConfig language.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Expr {
    /// Named symbol (terminal).
    Symbol(String),

    /// Integer constant (terminal).
    Integer(i64),

    /// Equality comparison.
    Eq(Box<Expr>, Box<Expr>),

    /// Inequality comparison.
    Ne(Box<Expr>, Box<Expr>),

    /// Less-than comparison.
    Lt(Box<Expr>, Box<Expr>),

    /// Less-than-or-equal comparison.
    Le(Box<Expr>, Box<Expr>),

    /// Greater-than comparison.
    Gt(Box<Expr>, Box<Expr>),

    /// Greater-than-or-equal comparison.
    Ge(Box<Expr>, Box<Expr>),

    /// Unary negation.
    Not(Box<Expr>),

    /// Boolean AND.
    And(Box<Expr>, Box<Expr>),

    /// Boolean OR.
    Or(Box<Expr>, Box<Expr>),
}

/// A terminal expression (symbol or constant) in the KConfig language.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ExprTerm {
    /// Named symbol.
    Symbol(String),

    /// Integer constant.
    Integer(i64),
}

/// A parsed but not yet precedence-resolved token in an expression.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ExprToken {
    /// Named symbol.
    Symbol(String),

    /// Integer constant.
    Integer(i64),

    /// Parenthesized expression.
    Paren(Vec<ExprToken>),

    /// Equality comparison.
    Eq,

    /// Inequality comparison.
    Ne,

    /// Less-than comparison.
    Lt,

    /// Less-than-or-equal comparison.
    Le,

    /// Greater-than comparison.
    Gt,

    /// Greater-than-or-equal comparison.
    Ge,

    /// Unary negation.
    Not,

    /// Boolean AND.
    And,

    /// Boolean OR.
    Or,
}

/// Parse, but do not precedence-resolve, an expression.
pub fn parse_expr<'a, E>(input: &'a str) -> IResult<&'a str, Vec<ExprToken>, E>
where
    E: ParseError<&'a str> + FromExternalError<&'a str, ParseIntError>,
{
    many0(parse_expr_token)(input)
}

/// Resolve a parsed expression into an expression tree.
fn resolve_expr(tokens: &[ExprToken]) -> Result<Expr, KConfigError> {
    todo!()
}

/// Parse an expression token.
fn parse_expr_token<'a, E>(input: &'a str) -> IResult<&'a str, ExprToken, E>
where
    E: ParseError<&'a str> + FromExternalError<&'a str, ParseIntError>,
{
    preceded(
        hws0,
        alt((
            map(parse_symbol, ExprToken::Symbol),
            map(parse_integer, ExprToken::Integer),
            map(tag("="), |_| ExprToken::Eq),
            map(tag("!="), |_| ExprToken::Ne),
            map(tag("<="), |_| ExprToken::Le),
            map(tag(">="), |_| ExprToken::Ge),
            map(tag("<"), |_| ExprToken::Lt),
            map(tag(">"), |_| ExprToken::Gt),
            map(tag("!"), |_| ExprToken::Not),
            map(tag("&&"), |_| ExprToken::And),
            map(tag("||"), |_| ExprToken::Or),
            map(delimited(char('('), many1(parse_expr_token), char(')')), ExprToken::Paren),
        )),
    )(input)
}

/// Parse a symbol.
fn parse_symbol<'a, E>(input: &'a str) -> IResult<&'a str, String, E>
where
    E: ParseError<&'a str>,
{
    map(recognize(pair(alt((alpha1::<&'a str, E>, tag("_"))), many0_count(alt((alphanumeric1, tag("_")))))), |s| {
        s.to_string()
    })(input)
}

/// Parse an integer.
fn parse_integer<'a, E>(input: &'a str) -> IResult<&'a str, i64, E>
where
    E: ParseError<&'a str> + FromExternalError<&'a str, ParseIntError>,
{
    alt((parse_decimal, parse_hex, parse_octal))(input)
}

/// Parse a decimal integer.
fn parse_decimal<'a, E>(input: &'a str) -> IResult<&'a str, i64, E>
where
    E: ParseError<&'a str> + FromExternalError<&'a str, ParseIntError>,
{
    map_res(recognize(tuple((opt(one_of("+-")), one_of("123456789"), many0_count(digit1)))), str::parse)(input)
}

/// Parse a hexadecimal integer.
fn parse_hex<'a, E>(input: &'a str) -> IResult<&'a str, i64, E>
where
    E: ParseError<&'a str> + FromExternalError<&'a str, ParseIntError>,
{
    preceded(alt((tag("0x"), tag("0X"))), map_res(hex_digit1, |s| i64::from_str_radix(s, 16)))(input)
}

/// Parse an octal integer.
fn parse_octal<'a, E>(input: &'a str) -> IResult<&'a str, i64, E>
where
    E: ParseError<&'a str> + FromExternalError<&'a str, ParseIntError>,
{
    map_res(recognize(tuple((opt(one_of("+-")), char('0'), oct_digit1))), |s| i64::from_str_radix(s, 8))(input)
}

#[cfg(test)]
mod tests {
    use super::{parse_expr, ExprToken};

    #[test]
    fn expr_basic() {
        let (rest, tokens) = parse_expr::<()>(
            "x && y || z && (w = a || b < 0x1e3 && c <= -0777 && d > +1234 && h >= -55 && !(i != 100))",
        )
        .unwrap();
        assert_eq!(rest, "");
        assert_eq!(
            tokens,
            vec![
                ExprToken::Symbol("x".into()),
                ExprToken::And,
                ExprToken::Symbol("y".into()),
                ExprToken::Or,
                ExprToken::Symbol("z".into()),
                ExprToken::And,
                ExprToken::Paren(vec![
                    ExprToken::Symbol("w".into()),
                    ExprToken::Eq,
                    ExprToken::Symbol("a".into()),
                    ExprToken::Or,
                    ExprToken::Symbol("b".into()),
                    ExprToken::Lt,
                    ExprToken::Integer(483),
                    ExprToken::And,
                    ExprToken::Symbol("c".into()),
                    ExprToken::Le,
                    ExprToken::Integer(-511),
                    ExprToken::And,
                    ExprToken::Symbol("d".into()),
                    ExprToken::Gt,
                    ExprToken::Integer(1234),
                    ExprToken::And,
                    ExprToken::Symbol("h".into()),
                    ExprToken::Ge,
                    ExprToken::Integer(-55),
                    ExprToken::And,
                    ExprToken::Not,
                    ExprToken::Paren(vec![
                        ExprToken::Symbol("i".into()),
                        ExprToken::Ne,
                        ExprToken::Integer(100),
                    ]),
                ]),
            ]
        );
    }
}
