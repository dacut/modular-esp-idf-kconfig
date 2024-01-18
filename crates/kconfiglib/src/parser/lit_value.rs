/// A literal value.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LitValue {
    /// Integer value.
    Int(i64),

    /// String value.
    String(String),

    /// Symbol.
    Symbol(String),

    /// Tristate value.
    Tristate(Tristate),
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
