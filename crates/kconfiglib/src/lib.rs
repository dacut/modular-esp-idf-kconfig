//! KConfig parsing and evaluation crate.
#![warn(clippy::all)]
#![allow(clippy::result_large_err)]
#![warn(missing_docs)]

mod context;
mod parser;
mod simplifier;
mod target;

pub use {context::*, parser::*, simplifier::*, target::*};

/// Default KConfigs.in for `COMPONENT_KCONFIGS_SOURCE_FILE`.
pub const KCONFIGS_IN: &str = include_str!("Kconfigs.in");

/// Default KConfigs.projbuild.in for `COMPONENT_KCONFIGS_PROJBUILD_SOURCE_FILE`.
pub const KCONFIGS_PROJBUILD_IN: &str = include_str!("Kconfigs.projbuild.in");
