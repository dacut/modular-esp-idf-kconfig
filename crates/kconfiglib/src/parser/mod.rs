//! KConfig parser.

mod block;
mod comment;
mod expr;
mod location;
mod string_literal;
mod token;
mod types;
mod whitespace;

use {
    nom::{combinator::all_consuming, error::VerboseError, multi::many0, Err as NomErr},
    std::{
        backtrace::Backtrace,
        error::Error,
        fmt::{Debug, Display, Formatter, Result as FmtResult},
        fs::File,
        io::{Error as IoError, Read},
        path::Path,
    },
};

pub(crate) use {block::*, comment::*, whitespace::*};
pub use {
    expr::{Expr, ExprTerm, parse_expr},
    location::Location,
    string_literal::parse_string_literal,
    token::Token,
    types::Type,
};

/// A parsed KConfig file.
#[derive(Debug, Default)]
pub struct KConfig {
    /// The blocks found in the file.
    pub blocks: Vec<Block>,
}

impl KConfig {
    /// Parse a KConfig file from the given string input.
    pub fn parse_str(input: &str) -> Result<Self, KConfigError> {
        let (_, blocks) = all_consuming(many0(parse_block::<VerboseError<&str>>))(input)?;

        let result = Self {
            blocks,
        };
        Ok(result)
    }

    /// Parse the given file.
    pub fn parse_filename(filename: impl AsRef<Path>) -> Result<Self, KConfigError> {
        let filename = filename.as_ref();
        let mut file = File::open(filename)?;
        let mut input = String::new();
        file.read_to_string(&mut input)?;
        Self::parse_str(input.as_str())
    }
}

/// An error that occurred while parsing a KConfig file.
#[derive(Debug)]
pub struct KConfigError {
    /// The kind of error that occurred.
    pub kind: KConfigErrorKind,

    /// Additional backtrace information.
    pub backtrace: Backtrace,
}

impl KConfigError {
    /// Create a new [KConfigError] with the given kind. The backtrace will be captured automatically.
    pub fn new(kind: KConfigErrorKind) -> Self {
        Self {
            kind,
            backtrace: Backtrace::capture(),
        }
    }
}

impl Display for KConfigError {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        Display::fmt(&self.kind, f)
    }
}

impl From<IoError> for KConfigError {
    fn from(e: IoError) -> Self {
        Self::new(KConfigErrorKind::Io(e))
    }
}

impl From<NomErr<&'_ str>> for KConfigError {
    fn from(e: NomErr<&'_ str>) -> Self {
        Self::new(KConfigErrorKind::Parse(format!("{}", e)))
    }
}

impl<I> From<NomErr<VerboseError<I>>> for KConfigError
where
    I: Debug,
{
    fn from(e: NomErr<VerboseError<I>>) -> Self {
        Self::new(KConfigErrorKind::Parse(format!("{}", e)))
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
    use super::KConfig;

    #[test]
    fn kconfig_comments_blank_lines() {
        let kconfig = KConfig::parse_str(
            r##"mainmenu "Hello, world!"

    source "/tmp/myfile"

    # Read the next file
    source "/tmp/myfile2"
"##,
        )
        .unwrap();

        assert_eq!(kconfig.blocks.len(), 3);
    }
}
