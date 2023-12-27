use std::fmt::{Display, Formatter, Result as FmtResult};

/// Symbol/choice types.
#[allow(missing_docs)]
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub enum Type {
    #[default]
    Unknown,
    Bool,
    Tristate,
    String,
    Int,
    Hex,
}

impl Display for Type {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        match self {
            Type::Unknown => write!(f, "unknown"),
            Type::Bool => write!(f, "bool"),
            Type::Tristate => write!(f, "tristate"),
            Type::String => write!(f, "string"),
            Type::Int => write!(f, "int"),
            Type::Hex => write!(f, "hex"),
        }
    }
}

impl Type {
    /// Return the integer base used for string representations of integers.
    /// 0 means the base is inferred from the format of the string.
    pub fn base(&self) -> u32 {
        match self {
            Type::Int => 10,
            Type::Hex => 16,
            _ => 0,
        }
    }
}
