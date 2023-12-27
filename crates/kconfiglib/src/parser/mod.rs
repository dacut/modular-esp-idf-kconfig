//! KConfig parser.

mod block;
mod line;
mod location;
mod string_literal;
mod token;
mod types;

use {
    nom_locate::LocatedSpan,
    std::{
        backtrace::Backtrace,
        error::Error,
        fmt::{Display, Formatter, Result as FmtResult},
        fs::File,
        io::{Error as IoError, Read},
        path::PathBuf,
    },
};

pub(crate) type Span<'a> = LocatedSpan<&'a str, PathBuf>;
pub(crate) use {block::*, line::*};
pub use {location::Location, string_literal::parse_string_literal, token::Token, types::Type};

/// Parse a KConfig file, returning a [KConfig] struct.
pub fn parse(filename: impl Into<PathBuf>) -> Result<KConfig, KConfigError> {
    let filename = filename.into();
    let mut file = File::open(&filename)?;
    let mut data = String::new();
    file.read_to_string(&mut data)?;
    drop(file);
    parse_data(filename, &data)
}

/// Parse a KConfig file with the specified contents, returning a [KConfig] struct.
pub fn parse_data(filename: impl Into<PathBuf>, data: &str) -> Result<KConfig, KConfigError> {
    let filename = filename.into();
    let location = Location {
        filename,
        line: 1,
        column: 1,
    };
    KConfig::parse_file(location, data)
}

/// A parsed KConfig file.
#[derive(Debug, Default)]
pub struct KConfig {
    /// The blocks found in the file.
    pub blocks: Vec<Block>,
}

impl KConfig {
    fn parse_file(mut location: Location, mut data: &str) -> Result<Self, KConfigError> {
        let mut blocks = Vec::with_capacity(16);
        loop {
            let block = match parse_block(location, data)? {
                None => break,
                Some(block) => block,
            };

            blocks.push(block.block);
            location = block.next_location;
            data = block.remaining;
        }

        Ok(Self {
            blocks,
        })
    }
}

/// An error that occurred while parsing a KConfig file.
#[derive(Debug)]
pub struct KConfigError {
    /// The kind of error that occurred.
    pub kind: KConfigErrorKind,

    /// The location of the error.
    pub location: Option<Location>,

    /// Additional backtrace information.
    pub backtrace: Backtrace,
}

impl KConfigError {
    /// Create a new [KConfigError] with the given kind and location. The backtrace will be captured automatically.
    pub fn new(kind: KConfigErrorKind, location: Option<Location>) -> Self {
        Self {
            kind,
            location,
            backtrace: Backtrace::capture(),
        }
    }
}

impl Display for KConfigError {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        if let Some(location) = &self.location {
            write!(f, "{}: ", location)?;
        }
        Display::fmt(&self.kind, f)
    }
}

impl From<IoError> for KConfigError {
    fn from(e: IoError) -> Self {
        Self::new(KConfigErrorKind::Io(e), None)
    }
}

impl Error for KConfigError {}

/// The types of errors that can occur while parsing a KConfig file.
#[derive(Debug)]
pub enum KConfigErrorKind {
    /// I/O error.
    Io(IoError),

    /// Generic parsing error.
    Parse(String),
}

impl Display for KConfigErrorKind {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        match self {
            Self::Io(e) => write!(f, "I/O error: {}", e),
            Self::Parse(e) => write!(f, "Parse error: {}", e),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::parser::{parse_data, Source};

    #[test]
    fn test_mainmenu_block() {
        let kconfig = parse_data(
            "myfile",
            r#"
mainmenu "This is the main menu"
"#,
        )
        .unwrap();
        assert_eq!(kconfig.blocks.len(), 1);

        assert_eq!(kconfig.blocks[0].kind, super::BlockKind::Mainmenu("This is the main menu".to_string()));
    }

    #[test]
    fn test_source_blocks() {
        let kconfig = parse_data(
            "myfile",
            r#"
source "/tmp/required"
osource "/tmp/optional"
rsource "relative"
orsource "relative_optional"
"#,
        )
        .unwrap();
        assert_eq!(kconfig.blocks.len(), 4);

        assert_eq!(
            kconfig.blocks[0].kind,
            super::BlockKind::Source(Source {
                filename: "/tmp/required".into(),
                optional: false,
                relative: false
            })
        );
        assert_eq!(
            kconfig.blocks[1].kind,
            super::BlockKind::Source(Source {
                filename: "/tmp/optional".into(),
                optional: true,
                relative: false
            })
        );
        assert_eq!(
            kconfig.blocks[2].kind,
            super::BlockKind::Source(Source {
                filename: "relative".into(),
                optional: false,
                relative: true
            })
        );
        assert_eq!(
            kconfig.blocks[3].kind,
            super::BlockKind::Source(Source {
                filename: "relative_optional".into(),
                optional: true,
                relative: true
            })
        );
    }
}
