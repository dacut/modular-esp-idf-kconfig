use {
    once_cell::sync::OnceCell,
    std::{
        cmp::{Eq, PartialEq},
        collections::HashMap,
        fmt::{Debug, Display, Formatter, Result as FmtResult, Write as FmtWrite},
        hash::{Hash, Hasher},
        ops::{Index, IndexMut},
        path::{Path, PathBuf},
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

/// A trait for items that might have location information.
pub trait GetLocation {
    /// Get the location of the item.
    fn get_location(&self) -> Option<Location>;
}

/// A wrapper for values that might have location information.
pub struct Located<T> {
    /// The value to associate with location information.
    pub inner: T,

    /// The location of the value.
    pub location: Option<Location>,
}

/// A [`String`] with optional location information.
pub type LocString = Located<String>;

/// A string slice ([`str`]) with optional location information.
pub type LocStr<'a> = Located<&'a str>;

/// A mutable string slice ([`str`]) with optional location information.
pub type LocMutStr<'a> = Located<&'a mut str>;

impl Location {
    /// Update the location from a string slice.
    pub fn update(mut self, s: &str) -> Self {
        for c in s.chars() {
            if c == '\n' {
                self.line += 1;
                self.column = 1;
            } else {
                self.column += 1;
            }
        }

        self
    }
}

impl<T> Located<T> {
    /// Create a new [`Located`] from a value and a location.
    #[inline(always)]
    pub fn new(value: T, location: Option<Location>) -> Self {
        Self {
            inner: value,
            location,
        }
    }

    /// Consume this [`Located`] and return the underlying value.
    #[inline(always)]
    pub fn into_inner(self) -> T {
        self.inner
    }
}

impl<T> AsRef<T> for Located<T> {
    #[inline(always)]
    fn as_ref(&self) -> &T {
        &self.inner
    }
}

impl<T> AsMut<T> for Located<T> {
    #[inline(always)]
    fn as_mut(&mut self) -> &mut T {
        &mut self.inner
    }
}

impl<T: Clone> Clone for Located<T> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
            location: self.location,
        }
    }
}

impl<T: Copy> Copy for Located<T> {}

impl<T: Debug> Debug for Located<T> {
    fn fmt(&self, f: &mut Formatter) -> FmtResult {
        if let Some(location) = self.location {
            write!(f, "{}: {:?}", location, self.inner)
        } else {
            Debug::fmt(&self.inner, f)
        }
    }
}

impl<T: Display> Display for Located<T> {
    fn fmt(&self, f: &mut Formatter) -> FmtResult {
        Display::fmt(&self.inner, f)
    }
}

impl<T: Eq> Eq for Located<T> {}

impl<T: FmtWrite> FmtWrite for Located<T> {
    fn write_str(&mut self, s: &str) -> FmtResult {
        self.inner.write_str(s)
    }
}

impl<T> GetLocation for Located<T> {
    fn get_location(&self) -> Option<Location> {
        self.location
    }
}

impl<T: Hash> Hash for Located<T> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.inner.hash(state);
    }
}

impl<T, I> Index<I> for Located<T>
where
    T: Index<I>,
{
    type Output = T::Output;

    fn index(&self, index: I) -> &Self::Output {
        Index::index(&self.inner, index)
    }
}

impl<T, I> IndexMut<I> for Located<T>
where
    T: IndexMut<I>,
{
    fn index_mut(&mut self, index: I) -> &mut Self::Output {
        IndexMut::index_mut(&mut self.inner, index)
    }
}

impl<T: Ord> Ord for Located<T> {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.inner.cmp(&other.inner)
    }
}

impl<T: PartialEq> PartialEq for Located<T> {
    fn eq(&self, other: &Self) -> bool {
        self.inner == other.inner
    }
}

impl<T: PartialOrd> PartialOrd for Located<T> {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        self.inner.partial_cmp(&other.inner)
    }
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
    /// Return a reference to the inner string slice with location information.
    pub fn to_loc_str(&self) -> LocStr {
        LocStr {
            inner: &self.inner,
            location: self.location,
        }
    }

    /// Return a mutable reference to the inner string slice with location information.
    pub fn to_loc_mut_str(&mut self) -> LocMutStr {
        LocMutStr {
            inner: &mut self.inner,
            location: self.location,
        }
    }
}

impl<'a> LocStr<'a> {
    /// Create a new [`LocString`] from this [`LocStr`].
    pub fn to_loc_string(&self) -> LocString {
        LocString {
            inner: self.inner.to_string(),
            location: self.location,
        }
    }
}

impl<'a> LocMutStr<'a> {
    /// Create a new [`LocString`] from this [`LocMutStr`].
    pub fn to_loc_string(&self) -> LocString {
        LocString {
            inner: self.inner.to_string(),
            location: self.location,
        }
    }
}

/// Cache of paths so we can return static references to them.
///
/// Paths are never evicted from this cache. Yes, it's a memory leak. No, we don't care.
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

impl GetLocation for &str {
    #[inline(always)]
    fn get_location(&self) -> Option<Location> {
        None
    }
}

impl GetLocation for String {
    #[inline(always)]
    fn get_location(&self) -> Option<Location> {
        None
    }
}

impl GetLocation for &String {
    #[inline(always)]
    fn get_location(&self) -> Option<Location> {
        None
    }
}
