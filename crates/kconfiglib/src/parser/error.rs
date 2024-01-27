use {
    crate::parser::Location,
    std::{
        backtrace::Backtrace,
        error::Error,
        fmt::{Debug, Display, Formatter, Result as FmtResult},
        io::Error as IoError,
    },
};

/// An error that occurred while parsing a KConfig file.
#[derive(Debug)]
pub struct KConfigError {
    /// The kind of error that occurred.
    pub kind: KConfigErrorKind,

    /// Additional backtrace information.
    pub backtrace: Backtrace,

    /// The location of the error.
    pub location: Option<Location>,
}

impl KConfigError {
    /// Create a new [KConfigError] with the given kind. The backtrace will be captured automatically.
    pub fn new(kind: KConfigErrorKind, location: Location) -> Self {
        Self {
            kind,
            backtrace: Backtrace::capture(),
            location: Some(location),
        }
    }

    /// Create a new [KConfigError] for an invalid environment variable.
    pub fn invalid_env(var: impl ToString, location: Location) -> Self {
        Self::new(KConfigErrorKind::InvalidEnv(var.to_string()), location)
    }

    /// Create a new [KConfigError] for an invalid integer literal.
    pub fn invalid_integer(value: impl ToString, location: Location) -> Self {
        Self::new(KConfigErrorKind::InvalidInteger(value.to_string()), location)
    }

    /// Create a new [KConfigError] for an invalid Unicode codepoint.
    pub fn invalid_unicode(codepoint: u32, location: Location) -> Self {
        Self::new(KConfigErrorKind::InvalidUnicode(codepoint), location)
    }

    /// Create a new [KConfigError] for a missing token.
    pub fn missing(expected: impl Into<Expected>, location: Location) -> Self {
        Self::new(KConfigErrorKind::Missing(expected.into()), location)
    }

    /// Create a new [KConfigError] for a syntax error.
    pub fn syntax(e: impl ToString, location: Location) -> Self {
        Self::new(KConfigErrorKind::Syntax(e.to_string()), location)
    }

    /// Create a new [KConfigError] for an unexpected character or string.
    pub fn unexpected(s: impl ToString, expected: impl Into<Expected>, location: Location) -> Self {
        Self::new(KConfigErrorKind::Unexpected(s.to_string(), expected.into()), location)
    }

    /// Create a new [KConfigError] for an unexpected end-of-file.
    pub fn unexpected_eof(expected: impl Into<Expected>, location: Location) -> Self {
        Self::new(KConfigErrorKind::UnexpectedEof(expected.into()), location)
    }

    /// Create a new [KConfigError] for an unknown environment variable.
    pub fn unknown_env(var: impl ToString, location: Location) -> Self {
        Self::new(KConfigErrorKind::UnknownEnv(var.to_string()), location)
    }
}

impl Display for KConfigError {
    fn fmt(&self, f: &mut Formatter) -> FmtResult {
        if let Some(loc) = &self.location {
            write!(f, "{}: {}", loc, self.kind)
        } else {
            write!(f, "{}", self.kind)
        }
    }
}

impl From<IoError> for KConfigError {
    fn from(e: IoError) -> Self {
        Self {
            kind: KConfigErrorKind::Io(e),
            backtrace: Backtrace::capture(),
            location: None,
        }
    }
}

impl Error for KConfigError {}

/// The types of errors that can occur while parsing a KConfig file.
#[derive(Debug)]
pub enum KConfigErrorKind {
    /// Invalid environment variable.
    InvalidEnv(String),

    /// Invalid integer literal.
    InvalidInteger(String),

    /// Invalid Unicode value.
    InvalidUnicode(u32),

    /// I/O error.
    Io(IoError),

    /// Missing a required token.
    Missing(Expected),

    /// Generic parsing error.
    Parse(String),

    /// Syntax error.
    Syntax(String),

    /// Expected a certain token, but got a different string.
    Unexpected(String, Expected),

    /// Expected a character of a certain type, but got end-of-file.
    UnexpectedEof(Expected),

    /// Unknown variable in filename expansion.
    UnknownEnv(String),
}

impl Display for KConfigErrorKind {
    fn fmt(&self, f: &mut Formatter) -> FmtResult {
        match self {
            Self::InvalidEnv(var) => write!(f, "Non-Unicode environment variable: {var}"),
            Self::InvalidInteger(value) => write!(f, "Invalid integer literal: {value}"),
            Self::InvalidUnicode(value) => write!(f, "Invalid Unicode value: \\u{{{value:x}}}"),
            Self::Io(e) => write!(f, "I/O error: {e}"),
            Self::Missing(expected) => write!(f, "Missing {expected}"),
            Self::Parse(e) => write!(f, "Parse error: {e}"),
            Self::Syntax(e) => write!(f, "Syntax error: {e}"),
            Self::Unexpected(s, expected) => {
                write!(f, "{s:?} unexpected; expected {expected}")
            }
            Self::UnexpectedEof(expected) => {
                if expected.is_any() {
                    write!(f, "Unexpected end-of-file")
                } else {
                    write!(f, "Unexpected end-of-file, expected {expected}")
                }
            }
            Self::UnknownEnv(var) => write!(f, "Unknown variable: {var}"),
        }
    }
}

/// Expected input description.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Expected {
    /// Any character.
    Any,

    /// Binary operator (`<=`, `>=`, `==`, `!=`, `<`, `>`, `&&`, `||`).
    BinOp,

    /// `endchoice` keyword.
    EndChoice,

    /// `endif` keyword.
    EndIf,

    /// `endmenu` keyword.
    EndMenu,

    /// `env` keyword.
    Env,

    /// End-of-line.
    Eol,

    /// Equals sign.
    Eq,

    /// Expression.
    Expr,

    /// `help` keyword.
    Help,

    /// ASCII hexadecimal digit.
    HexDigit,

    /// Keyword or symbol.
    KeywordOrSymbol,

    /// `if` keyword
    If,

    /// `if` or end-of-line.
    IfOrEol,

    /// An integer literal.
    IntegerLiteral,

    /// A literal value.
    LitValue,

    /// `on` keyword
    On,

    /// One of the given characters.
    OneOf(Vec<char>),

    /// Right parenthesis.
    RParen,

    /// A string literal.
    StringLiteral,

    /// A symbol.
    Symbol,

    /// A symbol or a value.
    SymbolOrValue,

    /// Unicode escape value.
    UnicodeEscape,

    /// Whitespace
    Whitespace,
}

impl Expected {
    /// Indicates if any character was expected.
    #[inline(always)]
    pub fn is_any(&self) -> bool {
        matches!(self, Self::Any)
    }
}

impl Display for Expected {
    fn fmt(&self, f: &mut Formatter) -> FmtResult {
        match self {
            Self::Any => f.write_str("any character"),
            Self::BinOp => f.write_str("binary operator"),
            Self::Eol => f.write_str("end of line"),
            Self::EndChoice => f.write_str("endchoice"),
            Self::EndIf => f.write_str("endif"),
            Self::EndMenu => f.write_str("endmenu"),
            Self::Env => f.write_str("env"),
            Self::Eq => f.write_str("="),
            Self::Expr => f.write_str("expression"),
            Self::Help => f.write_str("help"),
            Self::HexDigit => f.write_str("hexadecimal digit"),
            Self::KeywordOrSymbol => f.write_str("keyword or symbol"),
            Self::If => f.write_str("if"),
            Self::IfOrEol => f.write_str("if or end of line"),
            Self::IntegerLiteral => f.write_str("integer literal"),
            Self::LitValue => f.write_str("literal value"),
            Self::On => f.write_str("on"),
            Self::OneOf(v) => {
                write!(f, "one of: ")?;
                for (i, c) in v.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "'{}'", format_char(*c))?;
                }
                Ok(())
            }
            Self::RParen => f.write_str("right parenthesis"),
            Self::StringLiteral => f.write_str("string literal"),
            Self::Symbol => f.write_str("symbol"),
            Self::SymbolOrValue => f.write_str("symbol or value"),
            Self::UnicodeEscape => f.write_str("unicode escape sequence"),
            Self::Whitespace => f.write_str("whitespace"),
        }
    }
}

impl From<char> for Expected {
    fn from(c: char) -> Self {
        Self::OneOf(vec![c])
    }
}

impl From<Vec<char>> for Expected {
    fn from(v: Vec<char>) -> Self {
        Self::OneOf(v)
    }
}

impl From<&str> for Expected {
    fn from(s: &str) -> Self {
        Self::OneOf(s.chars().collect())
    }
}

fn format_char(c: char) -> String {
    if c.is_control() {
        c.escape_default().to_string()
    } else {
        c.to_string()
    }
}
