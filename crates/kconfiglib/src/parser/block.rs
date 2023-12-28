use {
    crate::parser::{eol, hws0, hws1, parse_string_literal},
    nom::{
        branch::alt,
        bytes::complete::tag,
        combinator::{map, value},
        error::{FromExternalError, ParseError},
        sequence::{separated_pair, terminated},
        IResult,
    },
    std::num::ParseIntError,
    std::path::PathBuf,
};

/// A Kconfig block.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Block {
    Choice,
    Comment,
    Config,
    Mainmenu(String),
    Menu,
    MenuConfig,
    Source(Source),
}

/// Source block type.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Source {
    pub filename: PathBuf,
    pub optional: bool,
    pub relative: bool,
}

/// Parse a block from `input`.
pub(crate) fn parse_block<'a, E>(input: &'a str) -> IResult<&'a str, Block, E>
where
    E: ParseError<&'a str> + FromExternalError<&'a str, ParseIntError>,
{
    let (input, _) = hws0(input)?;
    alt((parse_mainmenu, parse_source))(input)
}

/// Parse a `mainmenu` block from `input`.
pub(crate) fn parse_mainmenu<'a, E>(input: &'a str) -> IResult<&'a str, Block, E>
where
    E: ParseError<&'a str> + FromExternalError<&'a str, ParseIntError>,
{
    map(terminated(separated_pair(tag("mainmenu"), hws1, parse_string_literal), eol), |(_, menu)| Block::Mainmenu(menu))(
        input,
    )
}

/// Parse a `source`, `osource`, `rsource`, or `orsource` block from `input`.
pub(crate) fn parse_source<'a, E>(input: &'a str) -> IResult<&'a str, Block, E>
where
    E: ParseError<&'a str> + FromExternalError<&'a str, ParseIntError>,
{
    map(
        terminated(
            separated_pair(
                alt((
                    value((false, false), tag("source")),
                    value((true, false), tag("osource")),
                    value((false, true), tag("rsource")),
                    value((true, true), tag("orsource")),
                )),
                hws1,
                parse_string_literal,
            ),
            eol,
        ),
        |((optional, relative), filename)| {
            Block::Source(Source {
                filename: filename.into(),
                optional,
                relative,
            })
        },
    )(input)
}

#[cfg(test)]
mod tests {
    use crate::parser::{parse_block, Block, Source};

    #[test]
    fn test_valid_mainmenu() {
        let block = parse_block::<()>(r#"mainmenu "Hello world!""#).unwrap();
        assert_eq!(block, ("", Block::Mainmenu("Hello world!".into())));

        let block = parse_block::<()>(r#"mainmenu "Hello, \"world\"!""#).unwrap();
        assert_eq!(block, ("", Block::Mainmenu("Hello, \"world\"!".into())));

        let block = parse_block::<()>(r#"    mainmenu "Hello, world!"    "#).unwrap();
        assert_eq!(block, ("", Block::Mainmenu("Hello, world!".into())));

        let block = parse_block::<()>(r#"mainmenu "Hello, world!"\
    "#).unwrap();
        assert_eq!(block, ("", Block::Mainmenu("Hello, world!".into())));
    }

    #[test]
    fn test_valid_source() {
        let block = parse_block::<()>(r#"source "file"  "#).unwrap();
        assert_eq!(
            block,
            (
                "",
                Block::Source(Source {
                    filename: "file".into(),
                    optional: false,
                    relative: false,
                })
            )
        );

        let block = parse_block::<()>(r#"osource "file"  "#).unwrap();
        assert_eq!(
            block,
            (
                "",
                Block::Source(Source {
                    filename: "file".into(),
                    optional: true,
                    relative: false,
                })
            )
        );

        let block = parse_block::<()>(r#"rsource "file"  "#).unwrap();
        assert_eq!(
            block,
            (
                "",
                Block::Source(Source {
                    filename: "file".into(),
                    optional: false,
                    relative: true,
                })
            )
        );

        let block = parse_block::<()>(r#"orsource "file"  "#).unwrap();
        assert_eq!(
            block,
            (
                "",
                Block::Source(Source {
                    filename: "file".into(),
                    optional: true,
                    relative: true,
                })
            )
        );
    }
}