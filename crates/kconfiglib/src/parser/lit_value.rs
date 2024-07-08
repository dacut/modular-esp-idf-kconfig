use {
    crate::KConfigError,
    std::{
        cmp::{Ord, Ordering, PartialOrd},
        fmt::{Debug, Display, Formatter, Result as FmtResult},
        ops::Not,
    },
};

/// Literal value data.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LitValue {
    /// Hex value
    Hex(u64),

    /// Integer value.
    Int(i64),

    /// String value.
    String(String),

    /// Symbol.
    Symbol(String),

    /// Tristate value.
    Tristate(Tristate),
}

impl Display for LitValue {
    fn fmt(&self, f: &mut Formatter) -> FmtResult {
        match self {
            LitValue::Hex(v) => write!(f, "0x{:x}", v),
            LitValue::Int(v) => write!(f, "{v}"),
            LitValue::String(v) => Debug::fmt(v, f), // Escape the string
            LitValue::Symbol(v) => f.write_str(v),
            LitValue::Tristate(v) => Display::fmt(v, f),
        }
    }
}

/// A tristate value.
///
/// This takes on `true`, `false`, or `maybe`, corresponding with `y`, `n`, and `m`, respectively.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Tristate {
    /// `false` tristate value.
    False,

    /// `true` tristate value.
    True,

    /// `maybe` tristate value.
    Maybe,
}

impl Tristate {
    /// Convert a `Tristate` into an unsigned integer.
    ///
    /// This follows some esoteric Kconfig rules where:
    /// * [`False`][Tristate::False] is `0`
    /// * [`True`][Tristate::True] is `1`
    /// * [`Maybe`][Tristate::Maybe] is `2`
    #[inline(always)]
    pub fn to_u64(self) -> u64 {
        match self {
            Tristate::False => 0,
            Tristate::True => 1,
            Tristate::Maybe => 2,
        }
    }

    /// Convert a `Tristate` into an integer.
    ///
    /// This follows some esoteric Kconfig rules where:
    /// * [`False`][Tristate::False] is `0`
    /// * [`True`][Tristate::True] is `1`
    /// * [`Maybe`][Tristate::Maybe] is `2`
    #[inline(always)]
    pub fn to_i64(self) -> i64 {
        match self {
            Tristate::False => 0,
            Tristate::True => 1,
            Tristate::Maybe => 2,
        }
    }
}

impl Display for Tristate {
    fn fmt(&self, f: &mut Formatter) -> FmtResult {
        match self {
            Tristate::False => f.write_str("n"),
            Tristate::True => f.write_str("y"),
            Tristate::Maybe => f.write_str("m"),
        }
    }
}

impl From<bool> for Tristate {
    #[inline(always)]
    fn from(value: bool) -> Self {
        if value {
            Self::True
        } else {
            Self::False
        }
    }
}

impl TryFrom<&str> for Tristate {
    type Error = KConfigError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "n" => Ok(Tristate::False),
            "y" => Ok(Tristate::True),
            "m" => Ok(Tristate::Maybe),
            _ => Err(KConfigError::invalid_tristate(value, None)),
        }
    }
}

impl TryFrom<Tristate> for bool {
    type Error = TristateMaybe;

    #[inline(always)]
    fn try_from(value: Tristate) -> Result<bool, Self::Error> {
        match value {
            Tristate::False => Ok(false),
            Tristate::True => Ok(true),
            Tristate::Maybe => Err(TristateMaybe),
        }
    }
}

impl Not for Tristate {
    type Output = Self;

    #[inline(always)]
    fn not(self) -> Self::Output {
        match self {
            Tristate::False => Tristate::True,
            Tristate::True => Tristate::False,
            Tristate::Maybe => Tristate::Maybe,
        }
    }
}

impl Ord for Tristate {
    #[inline(always)]
    fn cmp(&self, other: &Self) -> Ordering {
        self.to_u64().cmp(&other.to_u64())
    }
}

impl PartialOrd for Tristate {
    #[inline(always)]
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// Error returned when converting a `Tristate` to a `bool` when the `Tristate` is `maybe`.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct TristateMaybe;
