use nom::{
    branch::alt,
    bytes::complete::tag,
    combinator::{eof, map, recognize},
    error::ParseError,
    multi::{fold_many0, fold_many1},
    IResult,
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
    let (input, _) = hws0(input)?;
    let (input, _) = nl_or_eof(input)?;
    Ok((input, ()))
}

#[cfg(test)]
mod tests {
    use crate::parser::{hws0, hws1};

    #[test]
    fn test_valid_hws0() {
        let (rest, ws) = hws0::<()>("").unwrap();
        assert_eq!(rest, "");
        assert_eq!(ws, "");

        let (rest, ws) = hws0::<()>(" ").unwrap();
        assert_eq!(rest, "");
        assert_eq!(ws, " ");

        let (rest, ws) = hws0::<()>("  ").unwrap();
        assert_eq!(rest, "");
        assert_eq!(ws, "  ");

        let (rest, ws) = hws0::<()>("  \t  ").unwrap();
        assert_eq!(rest, "");
        assert_eq!(ws, "  \t  ");

        let (rest, ws) = hws0::<()>("    \n").unwrap();
        assert_eq!(rest, "\n");
        assert_eq!(ws, "    ");

        let (rest, ws) = hws0::<()>("\t\\\na").unwrap();
        assert_eq!(rest, "a");
        assert_eq!(ws, "\t\\\n");

        let (rest, ws) = hws0::<()>("  \t  abcd").unwrap();
        assert_eq!(rest, "abcd");
        assert_eq!(ws, "  \t  ");

        let (rest, ws) = hws0::<()>("abcd  \t  abcd").unwrap();
        assert_eq!(rest, "abcd  \t  abcd");
        assert_eq!(ws, "");
    }

    #[test]
    fn test_valid_hws1() {
        hws1::<()>("").unwrap_err();

        let (rest, ws) = hws1::<()>(" ").unwrap();
        assert_eq!(rest, "");
        assert_eq!(ws, " ");

        let (rest, ws) = hws1::<()>("  ").unwrap();
        assert_eq!(rest, "");
        assert_eq!(ws, "  ");

        let (rest, ws) = hws1::<()>("  \t  ").unwrap();
        assert_eq!(rest, "");
        assert_eq!(ws, "  \t  ");

        let (rest, ws) = hws1::<()>("    \n").unwrap();
        assert_eq!(rest, "\n");
        assert_eq!(ws, "    ");

        let (rest, ws) = hws1::<()>("\t\\\na").unwrap();
        assert_eq!(rest, "a");
        assert_eq!(ws, "\t\\\n");

        let (rest, ws) = hws1::<()>("  \t  abcd").unwrap();
        assert_eq!(rest, "abcd");
        assert_eq!(ws, "  \t  ");

        hws1::<()>("abcd  \t  abcd").unwrap_err();
    }
}
