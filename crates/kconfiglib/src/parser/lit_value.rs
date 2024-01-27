use crate::parser::{Located, Location};

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

/// A literal value with a location.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LocLitValue {
    /// The literal value.
    pub value: LitValue,

    /// The location of the literal value.
    pub location: Location,
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

impl LocLitValue {
    /// Create a new `LocLitValue` from the given literal value and location.
    #[inline(always)]
    pub fn new(value: LitValue, location: Location) -> Self {
        Self {
            value,
            location,
        }
    }
}

impl Located for LocLitValue {
    fn location(&self) -> Location {
        self.location
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

/// Error returned when converting a `Tristate` to a `bool` when the `Tristate` is `maybe`.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct TristateMaybe;
