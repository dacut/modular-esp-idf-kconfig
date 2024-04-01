use {
    crate::{parser::KConfigError, parser::LocExpr, Context},
    std::path::Path,
};

/// A trait for adjusting the block hierarchy of a KConfig file.
///
/// This is used to:
/// * Load blocks that contain other blocks (`source` commands and similar)
/// * Propagate dependencies from `if` blocks onto other blocks.
pub trait ResolveBlock {
    /// The resulting type after the block is loaded.
    type Output: Sized;

    /// Resolve `source` commands and `if` blocks that encompass other blocks.
    /// 
    /// ## Parameters
    /// * `base_dir`: The base directory for the KConfig file.
    /// * `context`: The context for the KConfig file.
    /// * `parent_condition`: The condition for parent blocks. If there is no condition, this will be `true`.
    fn resolve_block<C>(&self, base_dir: &Path, context: &C, parent_condition: Option<&LocExpr>) -> Result<Self::Output, KConfigError>
    where
        C: Context;
}
