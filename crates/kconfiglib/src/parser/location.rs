use std::{
    fmt::{Debug, Display, Formatter, Result as FmtResult},
    path::PathBuf,
};

/// Location information for items in a Kconfig file.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Location {
    /// The file in which the item is located.
    pub filename: PathBuf,

    /// The line number of the item (1-based).
    pub line: usize,

    /// The column number of the item (0-based).
    pub column: usize,
}

impl Location {
    /// Create a new location from a filename, line number, and column number.
    pub fn new(filename: impl Into<PathBuf>, line: usize, column: usize) -> Self {
        Self {
            filename: filename.into(),
            line,
            column,
        }
    }
}

impl Display for Location {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        write!(f, "{} {}:{}", self.filename.display(), self.line, self.column)
    }
}

/// A located element.
pub struct Located<E> {
    /// The element being located.
    element: E,

    /// The location of the element.
    location: Location,
}

impl<E> Located<E> {
    /// Create a new located element.
    pub fn new(element: E, location: Location) -> Self {
        Self {
            element,
            location,
        }
    }

    /// Get the location.
    pub fn location(&self) -> &Location {
        &self.location
    }

    /// Rewraps the location around a method call on a reference to the element.
    #[inline(always)]
    pub fn map<'a, O>(&'a self, f: impl FnOnce(&'a E) -> O) -> Located<O> {
        Located {
            element: f(&self.element),
            location: self.location.clone(),
        }
    }

    /// Rewraps the location around a method call on the element itself.
    #[inline(always)]
    pub fn map_into<O>(self, f: impl FnOnce(E) -> O) -> Located<O> {
        Located {
            element: f(self.element),
            location: self.location,
        }
    }

    /// Converts the element into another type.
    #[inline(always)]
    pub fn into<T>(self) -> Located<T>
    where
        T: From<E>,
    {
        Located {
            element: self.element.into(),
            location: self.location,
        }
    }

    /// Consumes this located type and returns just the element.
    #[inline(always)]
    pub fn into_element(self) -> E {
        self.element
    }

    /// Splits this located element into its parts, element and location.
    #[inline(always)]
    pub fn into_parts(self) -> (E, Location) {
        (self.element, self.location)
    }
}

impl<E> Located<Option<E>> {
    /// Transpose a `Located<Option<E>>` into an `Option<Located<E>>`.
    pub fn transpose(self) -> Option<Located<E>> {
        match self.element {
            Some(element) => Some(Located {
                element,
                location: self.location,
            }),
            None => None,
        }
    }
}

impl<E> AsMut<E> for Located<E> {
    fn as_mut(&mut self) -> &mut E {
        &mut self.element
    }
}

impl<E> AsRef<E> for Located<E> {
    fn as_ref(&self) -> &E {
        &self.element
    }
}

impl<E> Clone for Located<E>
where
    E: Clone,
{
    fn clone(&self) -> Self {
        Self {
            element: self.element.clone(),
            location: self.location.clone(),
        }
    }
}

impl<E> Debug for Located<E>
where
    E: Debug,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        write!(f, "{:?}(at {})", self.element, self.location)
    }
}

impl<E> Display for Located<E>
where
    E: Display,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        write!(f, "{} (at {})", self.element, self.location)
    }
}

impl<E> Eq for Located<E> where E: Eq {}
impl<E> PartialEq for Located<E>
where
    E: PartialEq,
{
    fn eq(&self, other: &Self) -> bool {
        self.element == other.element
    }
}
