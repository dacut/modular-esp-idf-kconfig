use {
    crate::parser::parse_comment,
    nom::{
        branch::alt,
        bytes::complete::tag,
        combinator::{eof, map, opt, recognize},
        error::ParseError,
        multi::{fold_many0, fold_many1, many0},
        sequence::tuple,
        IResult,
    },
};

/// Horizontal whitespace, 0 or more spaces, tabs, or escaped newlines.
pub(crate) fn hws0<'a, E>(input: &'a str) -> IResult<&'a str, &'a str, E>
where
    E: ParseError<&'a str>,
{
    recognize(fold_many0(alt((tag(" "), tag("\t"), tag("\\\n"))), || 0, |a, _| a))(input)
}

/// Required horizontal whitespace, 0 or more spaces, tabs, or escaped newlines.
pub(crate) fn hws1<'a, E>(input: &'a str) -> IResult<&'a str, &'a str, E>
where
    E: ParseError<&'a str>,
{
    recognize(fold_many1(alt((tag(" "), tag("\t"), tag("\\\n"))), || 0, |a, _| a))(input)
}

/// Newline or end-of-input.
pub(crate) fn nl_or_eof<'a, E>(input: &'a str) -> IResult<&'a str, (), E>
where
    E: ParseError<&'a str>,
{
    map(alt((tag("\n"), eof)), |_| ())(input)
}

/// Logical end of line.
///
/// This is zero or more horizontal whitespace characters followed by a newline or end-of-input.
pub(crate) fn eol<'a, E>(input: &'a str) -> IResult<&'a str, (), E>
where
    E: ParseError<&'a str>,
{
    map(tuple((hws0, opt(parse_comment), nl_or_eof)), |(_, _, _)| ())(input)
}

/// Eat all whitespace at the start of input.
pub(crate) fn ws0<'a, E>(input: &'a str) -> IResult<&'a str, &'a str, E>
where
    E: ParseError<&'a str>,
{
    recognize(tuple((many0(eol), hws0)))(input)
}

#[cfg(test)]
mod tests {
    use crate::parser::{eol, parse_comment, hws0, hws1, ws0};

    #[test]
    fn hws0_empty() {
        let (rest, ws) = hws0::<()>("").unwrap();
        assert_eq!(rest, "");
        assert_eq!(ws, "");
    }

    #[test]
    fn hws0_simple() {
        let (rest, ws) = hws0::<()>(" ").unwrap();
        assert_eq!(rest, "");
        assert_eq!(ws, " ");

        let (rest, ws) = hws0::<()>("  ").unwrap();
        assert_eq!(rest, "");
        assert_eq!(ws, "  ");

        let (rest, ws) = hws0::<()>("  \t  ").unwrap();
        assert_eq!(rest, "");
        assert_eq!(ws, "  \t  ");
    }

    #[test]
    fn hws0_newline() {
        let (rest, ws) = hws0::<()>("    \n").unwrap();
        assert_eq!(rest, "\n");
        assert_eq!(ws, "    ");
    }

    #[test]
    fn hws0_escaped_newline() {
        let (rest, ws) = hws0::<()>("\t\\\na").unwrap();
        assert_eq!(rest, "a");
        assert_eq!(ws, "\t\\\n");
    }

    #[test]
    fn hws0_text() {
        let (rest, ws) = hws0::<()>("  \t  abcd").unwrap();
        assert_eq!(rest, "abcd");
        assert_eq!(ws, "  \t  ");

        let (rest, ws) = hws0::<()>("abcd  \t  abcd").unwrap();
        assert_eq!(rest, "abcd  \t  abcd");
        assert_eq!(ws, "");
    }

    #[test]
    fn hws1_empty() {
        hws1::<()>("").unwrap_err();
    }

    #[test]
    fn hws1_simple() {
        let (rest, ws) = hws1::<()>(" ").unwrap();
        assert_eq!(rest, "");
        assert_eq!(ws, " ");

        let (rest, ws) = hws1::<()>("  ").unwrap();
        assert_eq!(rest, "");
        assert_eq!(ws, "  ");

        let (rest, ws) = hws1::<()>("  \t  ").unwrap();
        assert_eq!(rest, "");
        assert_eq!(ws, "  \t  ");
    }

    #[test]
    fn hws1_newline() {
        let (rest, ws) = hws1::<()>("    \n").unwrap();
        assert_eq!(rest, "\n");
        assert_eq!(ws, "    ");
    }

    #[test]
    fn hws1_escaped_newline() {
        let (rest, ws) = hws1::<()>("\t\\\na").unwrap();
        assert_eq!(rest, "a");
        assert_eq!(ws, "\t\\\n");
    }

    #[test]
    fn hws1_text() {
        let (rest, ws) = hws1::<()>("  \t  abcd").unwrap();
        assert_eq!(rest, "abcd");
        assert_eq!(ws, "  \t  ");

        hws1::<()>("abcd  \t  abcd").unwrap_err();
    }

    #[test]
    fn comment_basic() {
        let (rest, _) = parse_comment::<()>(concat!(r#"# Hello world! "#, "\n")).unwrap();
        assert_eq!(rest, "\n");
    }

    #[test]
    fn eol_comment() {
        let (rest, _) = eol::<()>(concat!(r#"     # Hello world! "#, "\n\n")).unwrap();
        assert_eq!(rest, "\n");
    }

    #[test]
    fn ws0_lines() {
        let (rest, _) = ws0::<()>("\n    \n     Line\n").unwrap();
        assert_eq!(rest, "Line\n");
    }

    #[test]
    fn ws0_comments() {
        let (rest, _) = ws0::<()>("\n    # A comment \n     Line\n").unwrap();
        assert_eq!(rest, "Line\n");
    }
}
