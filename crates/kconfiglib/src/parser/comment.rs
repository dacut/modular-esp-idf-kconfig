use nom::{
    branch::alt,
    bytes::complete::{is_not, tag, take_while},
    character::streaming::char,
    combinator::{map, verify},
    error::ParseError,
    multi::fold_many0,
    sequence::{delimited, preceded},
    IResult,
};

/// Comment to end-of-line.
///
/// This is similar to parsing a string literal, but without delimiters.
pub(crate) fn parse_comment<'a, E>(input: &'a str) -> IResult<&'a str, (), E>
where
    E: ParseError<&'a str>,
{
    preceded(tag("#"), parse_comment_interior)(input)
}

/// Parse the interior of a comment (after the `#`).
fn parse_comment_interior<'a, E>(input: &'a str) -> IResult<&'a str, (), E>
where
    E: ParseError<&'a str>,
{
    fold_many0(parse_fragment, || 0, |a, _| a)(input).map(|(rest, _)| (rest, ()))
}

/// Parse a single fragment (escape or run of unescaped characters) in the comment.
fn parse_fragment<'a, E>(input: &'a str) -> IResult<&'a str, (), E>
where
    E: ParseError<&'a str>,
{
    alt((parse_literal, parse_escape))(input)
}

/// Parse a non-empty block of text that doesn't include \ or newline.
fn parse_literal<'a, E>(input: &'a str) -> IResult<&'a str, (), E>
where
    E: ParseError<&'a str>,
{
    let not_end = is_not("\\\n");
    map(verify(not_end, |s: &str| !s.is_empty()), |_| ())(input)
}

/// Parse an escape sequence
fn parse_escape<'a, E>(input: &'a str) -> IResult<&'a str, (), E>
where
    E: ParseError<&'a str>,
{
    map(
        preceded(
            tag("\\"),
            alt((
                // Unicode escape
                map(delimited(tag("u{"), take_while(|c: char| c.is_ascii_hexdigit()), tag("}")), |_| '\0'),
                // Regular escape sequences
                char('a'),
                char('b'),
                char('e'),
                char('f'),
                char('n'),
                char('r'),
                char('t'),
                char('v'),
                char('\\'),
                char('\''),
                char('/'),
                char('"'),
                char('\n'),
            )),
        ),
        |_| (),
    )(input)
}
