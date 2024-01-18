use std::{
    collections::{BTreeMap, HashMap},
    env::VarError,
};

/// A trait for performing variable lookups.
pub trait Context {
    /// Returns the value of the given variable, or an error if the variable could not be found.
    fn var(&self, name: &str) -> Result<String, VarError>;
}

/// A [context][Context] that uses the environment for variable lookups.
pub struct SystemContext;

impl Context for SystemContext {
    fn var(&self, name: &str) -> Result<String, VarError> {
        std::env::var(name)
    }
}

impl Context for BTreeMap<String, String> {
    fn var(&self, name: &str) -> Result<String, VarError> {
        self.get(name).cloned().ok_or(VarError::NotPresent)
    }
}

impl Context for HashMap<String, String> {
    fn var(&self, name: &str) -> Result<String, VarError> {
        self.get(name).cloned().ok_or(VarError::NotPresent)
    }
}

/// Create a closure around a context for [`env_with_context`][shellexpand::env_with_context].
pub(crate) fn context_closure<C>(context: &C) -> impl Fn(&str) -> Result<Option<String>, VarError> + '_
where
    C: Context,
{
    move |var| context.var(var).map(Some)
}
