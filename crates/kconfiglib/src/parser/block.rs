use {
    crate::{
        context_closure,
        parser::{
            cache_path, Choice, Config, Expected, Expr, KConfig, KConfigError, KConfigErrorKind, LocString, Located,
            Menu, PeekableChars, PeekableTokenLines, Token,
        },
        Context,
    },
    log::{debug, error, trace},
    shellexpand::env_with_context,
    slotmap::new_key_type,
    std::{env::VarError, io::ErrorKind as IoErrorKind, path::Path},
};

new_key_type! {
    /// Key for blocks in a KConfig file.
    pub struct BlockId;
}

/// The URL prefix for an inline source file.
const INLINE_PREFIX: &str = "inline:";

/// A block in a Kconfig file.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Block {
    /// Choice of configuration entries.
    Choice(Choice),

    /// Configuration entry for a symbol.
    Config(Config),

    /// Main menu title.
    Mainmenu(LocString),

    /// Menu block containing other items visible to the user in a submenu.
    Menu(Menu),

    /// Configuration entry for a symbol with an attached menu.
    MenuConfig(Config),
}

impl Block {
    /// If this is a choice block, return a reference to it; otherwise, return `None`.
    #[inline(always)]
    pub fn as_choice(&self) -> Option<&Choice> {
        match self {
            Block::Choice(c) => Some(c),
            _ => None,
        }
    }

    /// If this is a choice block, return a mutable reference to it; otherwise, return `None`.
    #[inline(always)]
    pub fn as_choice_mut(&mut self) -> Option<&mut Choice> {
        match self {
            Block::Choice(c) => Some(c),
            _ => None,
        }
    }

    /// If this is a config block, return a reference to it; otherwise, return `None`.
    #[inline(always)]
    pub fn as_config(&self) -> Option<&Config> {
        match self {
            Block::Config(c) => Some(c),
            _ => None,
        }
    }

    /// If this is a menu block, return a reference to it; otherwise, return `None`.
    #[inline(always)]
    pub fn as_menu(&self) -> Option<&Menu> {
        match self {
            Block::Menu(m) => Some(m),
            _ => None,
        }
    }

    /// If this is a menu block, return a mutable reference to it; otherwise, return `None`.
    #[inline(always)]
    pub fn as_menu_mut(&mut self) -> Option<&mut Menu> {
        match self {
            Block::Menu(m) => Some(m),
            _ => None,
        }
    }

    /// If this is a menuconfig block, return a reference to it; otherwise, return `None`.
    #[inline(always)]
    pub fn as_menuconfig(&self) -> Option<&Config> {
        match self {
            Block::MenuConfig(mc) => Some(mc),
            _ => None,
        }
    }

    /// If this is a config or menuconfig block, return a reference to the config; otherwise, return `None`.
    #[inline(always)]
    pub fn as_config_or_menuconfig(&self) -> Option<&Config> {
        match self {
            Block::Config(c) | Block::MenuConfig(c) => Some(c),
            _ => None,
        }
    }

    /// If this is a config or menuconfig block, return a reference to the config; otherwise, return `None`.
    #[inline(always)]
    pub fn into_config_or_menuconfig(self) -> Option<Config> {
        match self {
            Block::Config(c) | Block::MenuConfig(c) => Some(c),
            _ => None,
        }
    }

    /// Returns the parent block ID of this block, if it has one.
    pub fn get_parent(&self) -> Option<BlockId> {
        match self {
            Block::Choice(c) => c.parent,
            Block::Config(c) => c.parent,
            Block::Menu(m) => m.parent,
            Block::MenuConfig(c) => c.parent,
            Block::Mainmenu(_) => None,
        }
    }

    /// Parse the blocks from the given token lines into a KConfig.
    pub(crate) fn parse_blocks<C: Context>(
        kconfig: &mut KConfig,
        lines: &mut PeekableTokenLines,
        base_dir: &Path,
        parent_condition: Expr,
        parent: Option<BlockId>,
        context: &C,
    ) -> Result<Vec<BlockId>, KConfigError> {
        let mut block_ids = vec![];
        let mut last_pos = None;

        loop {
            let Some(tokens) = lines.peek() else {
                // End of file; we're done.
                break;
            };

            let Some(cmd) = tokens.peek() else {
                panic!("Expected block command");
            };

            if let Some(pos) = last_pos {
                if Some(pos) == cmd.location() {
                    panic!("No progress being made at {pos}");
                }
            }

            last_pos = cmd.location();

            match cmd.token {
                Token::Choice => {
                    block_ids.push(Choice::parse(kconfig, lines, base_dir, parent_condition.clone(), parent, context)?);
                }
                Token::Config | Token::MenuConfig => {
                    block_ids.push(Config::parse(kconfig, lines, base_dir, parent_condition.clone(), parent, context)?);
                }
                Token::If => {
                    block_ids.extend(Self::parse_if_block(
                        kconfig,
                        lines,
                        base_dir,
                        parent_condition.clone(),
                        parent,
                        context,
                    )?);
                }
                Token::Mainmenu => {
                    block_ids.push(Self::parse_mainmenu(kconfig, lines, base_dir, parent_condition.clone(), parent)?);
                }
                Token::Menu => {
                    block_ids.push(Menu::parse(kconfig, lines, base_dir, parent_condition.clone(), parent, context)?);
                }
                Token::Source | Token::OSource | Token::RSource | Token::ORSource => {
                    block_ids.extend(Self::parse_source(
                        kconfig,
                        lines,
                        base_dir,
                        parent_condition.clone(),
                        parent,
                        context,
                    )?);
                }
                Token::EndChoice | Token::EndMenu | Token::EndIf => {
                    // End of a choice, if, or menu block; we're done.
                    break;
                }
                _ => todo!("Block not handled: {cmd:?}"),
            };
        }

        Ok(block_ids)
    }

    /// Parse a `mainmenu` block.
    fn parse_mainmenu(
        kconfig: &mut KConfig,
        lines: &mut PeekableTokenLines,
        _base_dir: &Path,
        _parent_condition: Expr,
        _parent: Option<BlockId>,
    ) -> Result<BlockId, KConfigError> {
        let mut tokens = lines.next().unwrap();
        let (cmd, title) = tokens.read_cmd_str_lit(true)?;
        assert!(matches!(cmd.token, Token::Mainmenu));

        let block = Block::Mainmenu(title);
        Ok(kconfig.blocks.insert(block))
    }

    /// Parse an `if` block.
    ///
    /// These blocks are not recorded in the KConfig, but just push their conditions onto the sub-blocks within them.
    fn parse_if_block<C: Context>(
        kconfig: &mut KConfig,
        lines: &mut PeekableTokenLines,
        base_dir: &Path,
        parent_condition: Expr,
        parent: Option<BlockId>,
        context: &C,
    ) -> Result<Vec<BlockId>, KConfigError> {
        let mut tokens = lines.next().unwrap();
        assert!(!tokens.is_empty());

        let Some(if_token) = tokens.next() else {
            panic!("Expected if command");
        };
        assert!(matches!(if_token.token, Token::If));

        let condition = Expr::and(parent_condition, Expr::parse(if_token.location(), &mut tokens)?);

        if let Some(unexpected) = tokens.next() {
            return Err(KConfigError::unexpected(unexpected, Expected::Eol, unexpected.location()));
        }

        let block_ids = Self::parse_blocks(kconfig, lines, base_dir, condition, parent, context)?;

        let Some(mut line) = lines.next() else {
            return Err(KConfigError::unexpected_eof(Expected::EndIf, lines.location()));
        };

        let Some(end_if) = line.next() else {
            return Err(KConfigError::unexpected_eof(Expected::EndIf, lines.location()));
        };

        if end_if.token != Token::EndIf {
            return Err(KConfigError::unexpected(end_if, Expected::EndIf, end_if.location()));
        }

        Ok(block_ids)
    }

    /// Parse an `osource`, `orsource`, `rsource`, or `source` block.
    ///
    /// These blocks are not recorded in the KConfig directly; rather, the source file is opened and its contents are parsed.
    fn parse_source<C: Context>(
        kconfig: &mut KConfig,
        lines: &mut PeekableTokenLines,
        base_dir: &Path,
        parent_condition: Expr,
        parent: Option<BlockId>,
        context: &C,
    ) -> Result<Vec<BlockId>, KConfigError> {
        let mut tokens = lines.next().unwrap();
        assert!(!tokens.is_empty());

        let (cmd, filename) = tokens.read_cmd_str_lit(true)?;

        let optional = cmd.is_optional_source();
        let relative = cmd.is_relative_source();

        let base_dir = if relative {
            filename
                .location()
                .expect("Location must be present for relative source")
                .filename
                .parent()
                .unwrap_or_else(|| Path::new("/"))
        } else {
            base_dir
        }
        .to_path_buf();

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
            return kconfig.read_from_str(peek, &base_dir, parent_condition, parent, context);
        }

        let s_filename = base_dir.join(s_filename.as_ref());
        let s_filename = cache_path(&s_filename);

        trace!("Reading source file {s_filename:?}");
        match kconfig.read_from_file(s_filename, &base_dir, parent_condition, parent, context) {
            Ok(block_ids) => Ok(block_ids),
            Err(e) => {
                let KConfigErrorKind::Io(io_error) = &e.kind else {
                    error!("Unexpected non-I/O error while reading {s_filename:?}: {e}");
                    return Err(e);
                };

                if io_error.kind() != IoErrorKind::NotFound || !optional {
                    error!("Unable to read {s_filename:?}: {io_error}");
                    return Err(e);
                }

                debug!("Ignoring NotFound error for optional source file: {s_filename:?}");
                Ok(Vec::new())
            }
        }
    }
}
