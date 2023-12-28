use {
    crate::parser::{eol, hws1, parse_string_literal, ws0},
    nom::{
        branch::alt,
        bytes::complete::tag,
        combinator::{map, value},
        error::{FromExternalError, ParseError},
        sequence::{separated_pair, preceded, terminated},
        IResult,
    },
    std::{num::ParseIntError, path::PathBuf},
};

/// A Kconfig block.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Block {
    Choice,
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
    preceded(ws0, alt((parse_mainmenu, parse_source)))(input)
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
    fn plain_mainmenu() {
        let block = parse_block::<()>(concat!(r#"mainmenu "Hello world!""#, "\n")).unwrap();
        assert_eq!(block, ("", Block::Mainmenu("Hello world!".into())));
    }

    #[test]
    fn mainmenu_with_string_escapes() {
        let block = parse_block::<()>(concat!(r#"mainmenu "Hello, \"world\"!""#, "\n")).unwrap();
        assert_eq!(block, ("", Block::Mainmenu("Hello, \"world\"!".into())));
    }

    #[test]
    fn mainmenu_with_whitespace() {
        let block = parse_block::<()>(concat!(r#"    mainmenu "Hello, world!"    "#, "\n")).unwrap();
        assert_eq!(block, ("", Block::Mainmenu("Hello, world!".into())));
    }

    #[test]
    fn mainmenu_with_eol_continuation() {
        let block = parse_block::<()>("mainmenu \"Hello, world!\"\\\n    ").unwrap();
        assert_eq!(block, ("", Block::Mainmenu("Hello, world!".into())));
    }

    #[test]
    fn mainmenu_with_comment() {
        let block = parse_block::<()>(concat!(r#"mainmenu "Hello, world!" #Comment "#, "\n")).unwrap();
        assert_eq!(block, ("", Block::Mainmenu("Hello, world!".into())));
    }

    #[test]
    fn valid_source() {
        let (rest, block) = parse_block::<()>(r#"source "file"  "#,
        )
        .unwrap();
        assert_eq!(rest, "");
        assert_eq!(
            block,
                Block::Source(Source {
                    filename: "file".into(),
                    optional: false,
                    relative: false,
                })
        );
    }

    #[test]
    fn valid_osource() {
        let (rest, block) = parse_block::<()>(r#"osource "file"  "#).unwrap();
        assert_eq!(rest, "");
        assert_eq!(
            block,
                Block::Source(Source {
                    filename: "file".into(),
                    optional: true,
                    relative: false,
                })
        );
    }

    #[test]
    fn valid_rsource() {
        let (rest, block) = parse_block::<()>(r#"rsource "file"  "#).unwrap();
        assert_eq!(rest, "");
        assert_eq!(
            block,
                Block::Source(Source {
                    filename: "file".into(),
                    optional: false,
                    relative: true,
                })
        );
    }

    #[test]
    fn valid_orsource() {
        let (rest, block) = parse_block::<()>(r#"orsource "file"  "#).unwrap();
        assert_eq!(rest, "");
        assert_eq!(
            block,
                Block::Source(Source {
                    filename: "file".into(),
                    optional: true,
                    relative: true,
                })
        );
    }
}
