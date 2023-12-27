use {
    crate::parser::{read_nonempty_line, KConfigError, KConfigErrorKind, Location},
    lazy_regex::regex_captures,
    std::path::PathBuf,
};

/// A Kconfig block and its location.
#[derive(Debug)]
pub struct Block {
    pub kind: BlockKind,
    pub location: Location,
}

/// Kconfig block types.
#[derive(Debug, Eq, PartialEq)]
pub enum BlockKind {
    Choice,
    Comment,
    Config,
    Mainmenu(String),
    Menu,
    MenuConfig,
    Source(Source),
}

/// Source block type.
#[derive(Debug, Eq, PartialEq)]
pub struct Source {
    pub filename: PathBuf,
    pub optional: bool,
    pub relative: bool,
}

/// The result of reading a block, including the block itself, the remaining data, and the location of the next line.
#[derive(Debug)]
pub struct ReadBlockResult<'a> {
    pub block: Block,
    pub remaining: &'a str,
    pub next_location: Location,
}

/// Read a block from `data`, or return `None` if the end of the file is reached.
pub(crate) fn parse_block(mut location: Location, data: &str) -> Result<Option<ReadBlockResult<'_>>, KConfigError> {
    let Some(rlr) = read_nonempty_line(location.clone(), data) else {
        return Ok(None);
    };

    let line = rlr.line;
    let remaining = rlr.remaining;
    let next_location = rlr.next_location;

    // Match indentation (\s*) followed by:
    // * Empty line
    // * Comment (#.*)
    // * Command and possible arguments (?:([a-z_]+)(?:\s+)(.*))
    let (_, indent, comment, command, args) =
        match regex_captures!(r#"^(\s*)(#.*)|(?:([A-Za-z_][A-Za-z_0-9]*)(?:\s+)(.*))$"#, &line) {
            None => {
                return Err(KConfigError::new(
                    KConfigErrorKind::Parse(format!("Invalid start of block: {}", line)),
                    Some(location),
                ))
            }
            Some(m) => m,
        };

    location.advance(indent);

    if !comment.is_empty() {
        let block = Block {
            kind: BlockKind::Comment,
            location,
        };
        return Ok(Some(ReadBlockResult {
            block,
            remaining,
            next_location,
        }));
    }

    match command {
        "mainmenu" => parse_mainmenu(location, args, remaining, next_location),
        "osource" | "orsource" | "rsource" | "source" => {
            parse_source(location, command, args, remaining, next_location)
        }
        _ => Err(KConfigError::new(KConfigErrorKind::Parse(format!("Unknown command: {}", command)), Some(location))),
    }
}

fn parse_mainmenu<'a>(
    cmd_location: Location,
    args: &'_ str,
    remaining: &'a str,
    next_location: Location,
) -> Result<Option<ReadBlockResult<'a>>, KConfigError> {
    let (_, prompt) = match regex_captures!(r#"^\s*"((?:[^\\"]|\\.)+)"\s*$"#, args) {
        None => {
            return Err(KConfigError::new(
                KConfigErrorKind::Parse(format!("Expected prompt: {}", args)),
                Some(cmd_location),
            ))
        }
        Some(m) => m,
    };

    let block = Block {
        kind: BlockKind::Mainmenu(prompt.to_string()),
        location: cmd_location,
    };

    Ok(Some(ReadBlockResult {
        block,
        remaining,
        next_location,
    }))
}

fn parse_source<'a, 'b>(
    cmd_location: Location,
    command: &'a str,
    args: &'a str,
    remaining: &'b str,
    next_location: Location,
) -> Result<Option<ReadBlockResult<'b>>, KConfigError> {
    let (_, filename) = match regex_captures!(r#"^\s*"((?:[^\\"]|\\.)+)"\s*$"#, args) {
        None => {
            return Err(KConfigError::new(
                KConfigErrorKind::Parse(format!("Expected quoted filename: {}", args)),
                Some(cmd_location),
            ))
        }
        Some(m) => m,
    };

    let filename = PathBuf::from(filename);
    let optional = command == "osource" || command == "orsource";
    let relative = command == "orsource" || command == "rsource";
    let source = Source {
        filename,
        optional,
        relative,
    };

    let block = Block {
        kind: BlockKind::Source(source),
        location: cmd_location,
    };

    Ok(Some(ReadBlockResult {
        block,
        remaining,
        next_location,
    }))
}

#[cfg(test)]
mod tests {
    use {
        super::{parse_block, parse_source},
        crate::parser::{BlockKind, Location},
        std::path::PathBuf,
    };

    #[test]
    pub fn test_parse_source() {
        let cmd_location = Location {
            filename: PathBuf::from("myfile"),
            line: 1,
            column: 1,
        };
        let next_location = Location {
            filename: PathBuf::from("myfile"),
            line: 2,
            column: 1,
        };

        let result = parse_source(cmd_location, "source", "\"/tmp/required\"", "", next_location);
        assert!(result.is_ok());
        let result = result.unwrap();
        assert!(result.is_some());
        let result = result.unwrap();
        assert_eq!(
            result.block.kind,
            BlockKind::Source(super::Source {
                filename: PathBuf::from("/tmp/required"),
                optional: false,
                relative: false,
            })
        );
    }

    #[test]
    pub fn test_parse_block_source() {
        let location = Location {
            filename: PathBuf::from("myfile"),
            line: 1,
            column: 1,
        };

        let result = parse_block(location, "source \"/tmp/required\"\n");
        assert!(result.is_ok());
        let result = result.unwrap();
        assert!(result.is_some());
        let result = result.unwrap();
        assert_eq!(
            result.block.kind,
            BlockKind::Source(super::Source {
                filename: PathBuf::from("/tmp/required"),
                optional: false,
                relative: false,
            })
        );
    }
}
