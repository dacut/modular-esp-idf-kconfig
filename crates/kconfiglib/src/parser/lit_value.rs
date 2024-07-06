use std::fmt::{Debug, Display, Formatter, Result as FmtResult};
use std::ops::Not;

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

/// Error returned when converting a `Tristate` to a `bool` when the `Tristate` is `maybe`.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct TristateMaybe;
