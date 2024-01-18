use {
    crate::parser::{
        context::context_closure, Block, Context, KConfig, KConfigError, KConfigErrorKind, Located, LocatedBlocks,
        Token, TokenLine,
    },
    shellexpand::env_with_context,
    std::{
        env::VarError,
        io::ErrorKind as IoErrorKind,
        path::{Path, PathBuf},
    },
};

/// Source block type.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Source {
    /// The filename/glob pattern to read.
    pub filename: Located<String>,

    /// Whether the source statement is optional (`osource` or `orsource``).
    pub optional: bool,

    /// Whether the filename is relative to the current Kconfig file (`orsource` or `rsource`).
    pub relative: bool,

    /// The base directory for the source.
    pub base_dir: PathBuf,
}

impl Source {
    /// Parse a source line.
    pub fn parse(tokens: &mut TokenLine, base_dir: &Path) -> Result<Self, KConfigError> {
        let (cmd, filename) = tokens.read_cmd_str_lit(true)?;

        let optional = matches!(cmd.as_ref(), Token::OSource | Token::ORSource);
        let relative = matches!(cmd.as_ref(), Token::RSource | Token::ORSource);

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

    /// Evaluate the source directive and return new blocks found.
    pub fn evaluate<C>(&self, base_dir: &Path, context: &C) -> Result<Vec<Located<Block>>, KConfigError>
    where
        C: Context,
    {
        // Expand any ${ENV} variables in the filename.
        let s_filename = match env_with_context(self.filename.as_ref().as_str(), context_closure(context)) {
            Ok(s) => s,
            Err(e) => {
                return Err(match e.cause {
                    VarError::NotPresent => KConfigError::unknown_env(e.var_name, self.filename.location()),
                    VarError::NotUnicode(_) => KConfigError::invalid_env(e.var_name, self.filename.location()),
                })
            }
        };

        let s_filename = self.base_dir.join(s_filename.as_ref());
        log::debug!(
            "Vec<Located<Block>>::resolve_blocks_recursive: s_filename={s_filename:?}, optional={}",
            self.optional
        );

        match KConfig::parse_filename(&s_filename, base_dir) {
            Ok(mut s_kconfig) => {
                s_kconfig.resolve_blocks_recursive(base_dir, context)?;
                Ok(s_kconfig.blocks)
            }
            Err(e) => {
                log::error!("got error: {e}");
                let KConfigErrorKind::Io(io_error) = &e.kind else {
                    log::error!("Not an I/O error: {e}");
                    return Err(e);
                };

                if io_error.kind() != IoErrorKind::NotFound || !self.optional {
                    log::error!("Got IoError kind: {}", io_error.kind());
                    return Err(e);
                }

                log::debug!("Ignoring IoError of NotFound");
                Ok(Vec::new())
            }
        }
    }
}
