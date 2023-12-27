use {
    nom_locate::LocatedSpan,
    std::{
        fmt::{Display, Formatter, Result as FmtResult},
        path::PathBuf,
    },
};

/// Location information for items in a Kconfig file.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Location {
    /// The file in which the item is located.
    pub filename: PathBuf,

    /// The line number of the item (1-based).
    pub line: u32,

    /// The column number of the item (0-based).
    pub column: u32,
}

impl Location {
    /// Advance the location using the contents from the given string.
    #[inline(always)]
    pub fn advance(&mut self, s: &str) {
        for c in s.chars() {
            self.advance_char(c);
        }
    }

    /// Advance the location using the given character.
    pub fn advance_char(&mut self, c: char) {
        if c == '\n' {
            self.line += 1;
            self.column = 1;
        } else if c == '\t' {
            self.column = (self.column + 8) & !7;
        } else {
            self.column += 1;
        }
    }
}

impl Display for Location {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        write!(f, "{} {}:{}", self.filename.display(), self.line, self.column)
    }
}

impl From<LocatedSpan<&'_ str, PathBuf>> for Location {
    fn from(span: LocatedSpan<&'_ str, PathBuf>) -> Self {
        let line = span.location_line();
        let column = span.get_utf8_column().try_into().unwrap_or(u32::MAX);
        Self {
            filename: span.extra,
            line,
            column,
        }
    }
}

impl From<&LocatedSpan<&'_ str, PathBuf>> for Location {
    fn from(span: &LocatedSpan<&'_ str, PathBuf>) -> Self {
        let line = span.location_line();
        let column = span.get_utf8_column().try_into().unwrap_or(u32::MAX);
        Self {
            filename: span.extra.clone(),
            line,
            column,
        }
    }
}
