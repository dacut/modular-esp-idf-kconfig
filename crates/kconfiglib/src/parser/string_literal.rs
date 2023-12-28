//! String literal parsing. This is based on the example from
//! [nom](https://github.com/rust-bakery/nom/blob/main/examples/string.rs).
//!
//! A string is enclosed by double quotes (`"`) and can contain zero or more fragments consisting of:
//! * Any raw unescaped codepoint except `\\`` and `"`.
//! * One of the following escape sequences: `\\a`, `\\b`, `\\f`, `\\n`, `\\r`, `\\t`, `\\v`, `\\"`, `\\\\`
//! * A whitespace escape sequence of the form `\\[ \t\v\f]`.
//! * An octal escape sequence of the form `\\[0-7]{3}`.
//! * A hex escape sequence of the form `\\x[0-9a-fA-F]{2}`.
//! * A unicode escape sequence of the form `\\u{[0-9a-fA-F]{1,6}}`.

use {
    nom::{
        branch::alt,
        bytes::complete::{is_not, tag, take_while_m_n},
        character::streaming::{char, multispace1},
        combinator::{map, map_opt, map_res, value, verify},
        error::{FromExternalError, ParseError},
        multi::fold_many0,
        sequence::{delimited, preceded},
        IResult, Parser,
    },
    std::num::ParseIntError,
};

#[derive(Clone, Debug, Eq, PartialEq)]
enum StringFragment<'a> {
    Literal(&'a str),
    EscapedChar(char),
    EscapedWS,
}

/// Parse a string literal.
pub fn parse_string_literal<'a, E>(input: &'a str) -> IResult<&'a str, String, E>
where
    E: ParseError<&'a str> + FromExternalError<&'a str, ParseIntError>,
{
    // Run the fragment parser in a loop, building the output value onto an initially empty string.
    let build_string = fold_many0(
        // Our parser function– parses a single string fragment
        parse_fragment,
        // Our init value, an empty string
        String::new,
        // Our folding function. For each fragment, append the fragment to the
        // string.
        |mut string, fragment| {
            match fragment {
                StringFragment::Literal(s) => string.push_str(s),
                StringFragment::EscapedChar(c) => string.push(c),
                StringFragment::EscapedWS => {}
            }
            string
        },
    );

    // Finally, parse the delimited string.
    delimited(tag("\""), build_string, tag("\"")).parse(input)
}

/// Parse a single character in the string. This dispatches to the appropriate handler based on the input.
fn parse_fragment<'a, E>(input: &'a str) -> IResult<&'a str, StringFragment<'a>, E>
where
    E: ParseError<&'a str> + FromExternalError<&'a str, ParseIntError>,
{
    alt((
        // The `map` combinator runs a parser, then applies a function to the output
        // of that parser.
        map(parse_literal, StringFragment::Literal),
        map(parse_escaped_char, StringFragment::EscapedChar),
        value(StringFragment::EscapedWS, parse_escaped_whitespace),
    ))
    .parse(input)
}

/// Parse a non-empty block of text that doesn't include \ or "
fn parse_literal<'a, E>(input: &'a str) -> IResult<&'a str, &'a str, E>
where
    E: ParseError<&'a str>,
{
    // `is_not` parses a string of 0 or more characters that aren't one of the
    // given characters.
    let not_quote_slash = is_not("\"\\");

    // `verify` runs a parser, then runs a verification function on the output of
    // the parser. The verification function accepts out output only if it
    // returns true. In this case, we want to ensure that the output of is_not
    // is non-empty.
    verify(not_quote_slash, |s: &str| !s.is_empty()).parse(input)
}

/// Parse an escaped character: \n, \t, \r, \u{00AC}, etc.
fn parse_escaped_char<'a, E>(input: &'a str) -> IResult<&'a str, char, E>
where
    E: ParseError<&'a str> + FromExternalError<&'a str, ParseIntError>,
{
    preceded(
        char('\\'),
        // `alt` tries each parser in sequence, returning the result of
        // the first successful match
        alt((
            parse_unicode,
            // The `value` parser returns a fixed value (the first argument) if its
            // parser (the second argument) succeeds. In these cases, it looks for
            // the marker characters (n, r, t, etc) and returns the matching
            // character (\n, \r, \t, etc).
            value('\u{07}', char('a')), // alarm (BEL)
            value('\u{08}', char('b')), // backspace (BS)
            value('\u{1B}', char('e')), // escape (ESC)
            value('\u{0C}', char('f')), // form feed (FF)
            value('\n', char('n')),     // newline (LF)
            value('\r', char('r')),     // carriage return (CR)
            value('\t', char('t')),     // horizontal tab (TAB)
            value('\u{0B}', char('v')), // vertical tab (VT)
            value('\\', char('\\')),    // backslash
            value('\'', char('\'')),    // single quote
            value('/', char('/')),      // forward slash
            value('"', char('"')),      // double quote
        )),
    )
    .parse(input)
}

/// Parse a backslash, followed by any amount of whitespace. This is used later
/// to discard any escaped whitespace.
fn parse_escaped_whitespace<'a, E>(input: &'a str) -> IResult<&'a str, (), E>
where
    E: ParseError<&'a str>,
{
    value((), preceded(char('\\'), multispace1)).parse(input)
}

/// Parse a unicode sequence, of the form u{XXXX}, where XXXX is 1 to 6 hexadecimal numerals.
fn parse_unicode<'a, E>(input: &'a str) -> IResult<&'a str, char, E>
where
    E: ParseError<&'a str> + FromExternalError<&'a str, ParseIntError>,
{
    // parse_hex takes 1-6 hex digits.
    let parse_hex = take_while_m_n(1, 6, |c: char| c.is_ascii_hexdigit());

    // parse_delimited_hex takes a u, {, parse_hex, and }.
    let parse_delimited_hex = preceded(char::<_, E>('u'), delimited(char('{'), parse_hex, char('}')));

    // parse_u32 maps the result of parse_delimited_hex to a u32.
    let parse_u32 = map_res(parse_delimited_hex, move |hex| u32::from_str_radix(hex, 16));

    // Try to convert from u32 to char. This can fail if the u32 is not a valid Unicode codepoint, so map_opt
    // is used to return an error.
    map_opt(parse_u32, char::from_u32).parse(input)
}

#[cfg(test)]
mod tests {
    use super::parse_string_literal;

    #[test]
    fn test_parse_string_literal() {
        let (rest, value) = parse_string_literal::<'_, ()>(r#""Hello, world!""#).unwrap();
        assert_eq!(rest, "");
        assert_eq!(value, "Hello, world!");

        let (rest, value) = parse_string_literal::<'_, ()>(r#""Hello, \"world\"!""#).unwrap();
        assert_eq!(rest, "");
        assert_eq!(value, "Hello, \"world\"!");

        let (rest, value) = parse_string_literal::<'_, ()>(r#""Hello, \nworld!""#).unwrap();
        assert_eq!(rest, "");
        assert_eq!(value, "Hello, \nworld!");

        let (rest, value) = parse_string_literal::<'_, ()>(r#""Hello, \nworld!""#).unwrap();
        assert_eq!(rest, "");
        assert_eq!(value, "Hello, \nworld!");

        let (rest, value) = parse_string_literal::<'_, ()>(r#""Hello, \u{1F600}world!""#).unwrap();
        assert_eq!(rest, "");
        assert_eq!(value, "Hello, 😀world!");
    }
}
