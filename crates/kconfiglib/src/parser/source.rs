use {
    crate::{
        context_closure,
        parser::{
            cache_path, Block, KConfig, KConfigError, KConfigErrorKind, LocExpr, LocString, Located, PeekableChars,
            TokenLine,
        },
        Context, ResolveBlock,
    },
    log::{debug, error, trace},
    shellexpand::env_with_context,
    std::{
        cell::RefCell,
        env::VarError,
        io::ErrorKind as IoErrorKind,
        path::{Path, PathBuf},
        rc::Rc,
    },
};

/// Source block type.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Source {
    /// The filename/glob pattern to read.
    pub filename: LocString,

    /// Whether the source statement is optional (`osource` or `orsource``).
    pub optional: bool,

    /// Whether the filename is relative to the current Kconfig file (`orsource` or `rsource`).
    pub relative: bool,

    /// The base directory for the source.
    pub base_dir: PathBuf,
}

/// The URL prefix for an inline source file.
const INLINE_PREFIX: &str = "inline:";

impl Source {
    /// Parse a source line.
    pub fn parse(tokens: &mut TokenLine, base_dir: &Path) -> Result<Self, KConfigError> {
        let (cmd, filename) = tokens.read_cmd_str_lit(true)?;

        let optional = cmd.is_optional_source();
        let relative = cmd.is_relative_source();

        let base_dir = if relative {
            filename.location().filename.parent().unwrap_or_else(|| Path::new("/"))
        } else {
            base_dir
        }
        .to_path_buf();

        Ok(Source {
            filename,
            optional,
            relative,
            base_dir,
        })
    }
}

impl ResolveBlock for Source {
    type Output = Vec<Rc<RefCell<Block>>>;

    /// Evaluate the source directive and return new blocks found.
    fn resolve_block<C>(
        &self,
        base_dir: &Path,
        context: &C,
        parent_cond: Option<&LocExpr>,
    ) -> Result<Vec<Rc<RefCell<Block>>>, KConfigError>
    where
        C: Context,
    {
        let self_base_dir = &self.base_dir;
        let filename = &self.filename;

        // Expand any ${ENV} variables in the filename.
        let s_filename = match env_with_context(filename.as_str(), context_closure(context)) {
            Ok(s) => s,
            Err(e) => {
                return Err(match e.cause {
                    VarError::NotPresent => KConfigError::unknown_env(e.var_name, filename.location()),
                    VarError::NotUnicode(_) => KConfigError::invalid_env(e.var_name, filename.location()),
                })
            }
        };

        // Check if the filename is an inline source file, read from memory via the context.s
        if let Some(source) = s_filename.strip_prefix(INLINE_PREFIX) {
            // Read the source file from the context.
            let inline = cache_path(Path::new(INLINE_PREFIX));

            let peek = PeekableChars::new(source, inline);
            let s_kconfig = KConfig::from_str_raw(peek, base_dir, context)?;
            let s_kconfig = s_kconfig.resolve_block(base_dir, context, parent_cond)?;
            return Ok(s_kconfig.blocks);
        }

        let base_dir = if self.relative {
            // Relative to this source.
            self_base_dir
        } else {
            // Relative to the current base directory.
            base_dir
        };

        let s_filename = base_dir.join(s_filename.as_ref());
        let s_filename = cache_path(&s_filename);

        trace!("Reading source file {s_filename:?}");
        match KConfig::from_file(s_filename, base_dir, context) {
            Ok(s_kconfig) => Ok(s_kconfig.blocks),
            Err(e) => {
                let KConfigErrorKind::Io(io_error) = &e.kind else {
                    error!("Unexpected non-I/O error while reading {s_filename:?}: {e}");
                    return Err(e);
                };

                if io_error.kind() != IoErrorKind::NotFound || !self.optional {
                    error!("Unable to read {s_filename:?}: {io_error}");
                    return Err(e);
                }

                debug!("Ignoring NotFound error for optional source file: {s_filename:?}");
                Ok(Vec::new())
            }
        }
    }
}
