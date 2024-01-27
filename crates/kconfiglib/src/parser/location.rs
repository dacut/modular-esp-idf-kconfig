use {
    once_cell::sync::OnceCell,
    std::{
        cmp::{Eq, Ord, PartialEq, PartialOrd},
        collections::HashMap,
        fmt::{Debug, Display, Formatter, Result as FmtResult},
        hash::{Hash, Hasher},
        ops::{Deref, DerefMut},
        path::{Path, PathBuf},
        string::ToString,
        sync::Mutex,
    },
};

/// Location information for items in a Kconfig file.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Location {
    /// The file in which the item is located.
    pub filename: &'static Path,

    /// The line number of the item (1-based).
    pub line: usize,

    /// The column number of the item (0-based).
    pub column: usize,
}

/// A trait for items with location information.
pub trait Located {
    /// Get the location of the item.
    fn location(&self) -> Location;
}

/// A [`String`] with location information.
#[derive(Clone)]
pub struct LocString {
    value: String,
    location: Location,
}

/// A string slice ([`str`]) with location information.
#[derive(Clone, Copy)]
pub struct LocStr<'sl> {
    value: &'sl str,
    location: Location,
}

impl Location {
    /// Create a new location from a filename, line number, and column number.
    #[inline(always)]
    pub fn new(filename: &Path, line: usize, column: usize) -> Self {
        Self {
            filename: cache_path(filename),
            line,
            column,
        }
    }
}

impl Display for Location {
    #[inline(always)]
    fn fmt(&self, f: &mut Formatter) -> FmtResult {
        write!(f, "{} {}:{}", self.filename.display(), self.line, self.column)
    }
}

impl LocString {
    /// Create a new [`LocString`] from a [`String`] and a [`Location`].
    #[inline(always)]
    pub fn new(value: String, location: Location) -> Self {
        Self {
            value,
            location,
        }
    }

    /// Consume this [`LocString`] and return the underlying [`String`].
    #[inline(always)]
    pub fn into_inner(self) -> String {
        self.value
    }
}

impl Deref for LocString {
    type Target = String;

    #[inline(always)]
    fn deref(&self) -> &Self::Target {
        &self.value
    }
}

impl DerefMut for LocString {
    #[inline(always)]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.value
    }
}

impl Located for LocString {
    #[inline(always)]
    fn location(&self) -> Location {
        self.location
    }
}

impl Debug for LocString {
    #[inline(always)]
    fn fmt(&self, f: &mut Formatter) -> FmtResult {
        write!(f, "{:?}", self.value)
    }
}

impl Display for LocString {
    #[inline(always)]
    fn fmt(&self, f: &mut Formatter) -> FmtResult {
        write!(f, "{}", self.value)
    }
}

impl Eq for LocString {}
impl PartialEq for LocString {
    #[inline(always)]
    fn eq(&self, other: &Self) -> bool {
        self.value == other.value
    }
}

impl PartialEq<String> for LocString {
    #[inline(always)]
    fn eq(&self, other: &String) -> bool {
        self.value == *other
    }
}

impl Hash for LocString {
    #[inline(always)]
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.value.hash(state);
    }
}

impl Ord for LocString {
    #[inline(always)]
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.value.cmp(&other.value)
    }
}

impl PartialOrd for LocString {
    #[inline(always)]
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialOrd<String> for LocString {
    #[inline(always)]
    fn partial_cmp(&self, other: &String) -> Option<std::cmp::Ordering> {
        self.value.partial_cmp(other)
    }
}

impl<'sl> LocStr<'sl> {
    /// Create a new [`LocStr`] from a string slice and a [`Location`].
    #[inline(always)]
    pub fn new(value: &'sl str, location: Location) -> Self {
        Self {
            value,
            location,
        }
    }

    /// Consume this [`LocStr`] and return the underlying string slice.
    #[inline(always)]
    pub fn into_inner(self) -> &'sl str {
        self.value
    }

    /// Convert this into a [`LocString`].
    pub fn to_loc_string(&self) -> LocString {
        LocString::new(self.value.to_string(), self.location)
    }
}

impl Deref for LocStr<'_> {
    type Target = str;

    #[inline(always)]
    fn deref(&self) -> &Self::Target {
        self.value
    }
}

impl Located for LocStr<'_> {
    #[inline(always)]
    fn location(&self) -> Location {
        self.location
    }
}

impl Debug for LocStr<'_> {
    #[inline(always)]
    fn fmt(&self, f: &mut Formatter) -> FmtResult {
        Debug::fmt(self.value, f)
    }
}

impl Display for LocStr<'_> {
    #[inline(always)]
    fn fmt(&self, f: &mut Formatter) -> FmtResult {
        Display::fmt(self.value, f)
    }
}

impl Eq for LocStr<'_> {}
impl PartialEq for LocStr<'_> {
    #[inline(always)]
    fn eq(&self, other: &Self) -> bool {
        self.value == other.value
    }
}

impl PartialEq<str> for LocStr<'_> {
    #[inline(always)]
    fn eq(&self, other: &str) -> bool {
        self.value == other
    }
}

impl Hash for LocStr<'_> {
    #[inline(always)]
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.value.hash(state);
    }
}

impl Ord for LocStr<'_> {
    #[inline(always)]
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.value.cmp(other.value)
    }
}

impl PartialOrd for LocStr<'_> {
    #[inline(always)]
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialOrd<str> for LocStr<'_> {
    #[inline(always)]
    fn partial_cmp(&self, other: &str) -> Option<std::cmp::Ordering> {
        self.value.partial_cmp(other)
    }
}

static PATH_CACHE: OnceCell<Mutex<HashMap<PathBuf, &'static PathBuf>>> = OnceCell::new();

/// Return the cached path for the given path.
pub fn cache_path<P: Into<PathBuf>>(path: P) -> &'static Path {
    let map_mutex = PATH_CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    let path = path.into();

    // Get a mutex to the map.
    let mut map = map_mutex.lock().unwrap();

    // Do we already have an entry for this path?
    if let Some(ptr) = map.get(&path) {
        // Yes, return it.
        return ptr;
    }

    // No; allocate a new entry. We do this by leaking an allocation on the heap.
    let ptr = Box::leak(Box::new(path.clone()));
    map.insert(path, ptr);

    // Return the new entry.
    ptr
}
