use {
    crate::{
        parser::{
            Block, BlockId, Expected, Expr, KConfig, KConfigError, LitValue, LocString, Located, PeekableTokenLines,
            Prompt, Token, TokenLine, Tristate, Type,
        },
        Context,
    },
    log::warn,
    std::path::Path,
};

/// Configuration entry.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Config {
    /// The name of the symbol for this config block.
    pub name: LocString,

    /// The type of this config block.
    pub r#type: Type,

    /// The prompt for this config.
    pub prompt: Option<Prompt>,

    /// Help text for this config.
    pub help: Option<LocString>,

    /// Comments for this config.
    pub comments: Vec<LocString>,

    /// Default values for the config.
    pub defaults: Vec<ConfigDefault>,

    /// Environment variable to use as the default for this config.
    pub env: Option<LocString>,

    /// Dependencies for this config from `depend on` statements.
    pub depends_on: Expr,

    /// Other configs that are selected by this config.
    pub selects: Vec<ConfigTarget>,

    /// Other configs that are implied by this config.
    pub implies: Vec<ConfigTarget>,

    /// Range of acceptable values for this config.
    pub ranges: Vec<ConfigRange>,

    /// The parent block of this config, if it's not a top-level config.
    pub parent: Option<BlockId>,
}

/// Possible default for a configuration entry.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConfigDefault {
    /// The value of the default.
    pub value: Expr,

    /// The condition for this default. If unspecified in the Kconfig, this is `Expr::Bool(true)`.
    pub condition: Expr,
}

/// The target of a `select` or `imply` statement along with an optional associated condition.
///
/// These statements are in one of the following forms:
/// * `select TARGET`
/// * `select TARGET if EXPR`
/// * `imply TARGET`
/// * `imply TARGET if EXPR`
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConfigTarget {
    /// The name of the target of this `select` or `imply` statement.
    pub target_name: String,

    /// The condition for this `select` or `imply` statement. If unspecified in the Kconfig, this is `Expr::Tristate(Tristate::True)`.
    pub condition: Expr,
}

/// Range for a configuration entry.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConfigRange {
    /// The starting value of the range.
    pub start: LitValue,

    /// The ending value of the range.
    pub end: LitValue,

    /// The condition for this range. If unspecified in the Kconfig, this is `Expr::Tristate(Tristate::True)`.
    pub condition: Expr,
}

impl Config {
    /// Parse a `config` block.
    ///
    /// Parameters:
    /// * `kconfig`: The KConfig instance to add this config to.
    /// * `lines`: The lines to parse. The first line must start with a [`Token::Config`] token.
    /// * `_base_dir`: The base directory for the KConfig file (ignored).
    /// * `parent_condition`: The condition that must be true for this config to be included.
    /// * `parent`: The parent block of this config, if it's not a top-level config.
    pub fn parse<C: Context>(
        kconfig: &mut KConfig,
        lines: &mut PeekableTokenLines,
        _base_dir: &Path,
        parent_condition: Expr,
        parent: Option<BlockId>,
        _context: &C,
    ) -> Result<BlockId, KConfigError> {
        let Some(mut tokens) = lines.next() else {
            panic!("Expected config block");
        };

        let (blk_cmd, name) = tokens.read_cmd_sym(true)?;

        assert!(
            matches!(blk_cmd.token, Token::Config | Token::MenuConfig),
            "Expected config or menuconfig: {blk_cmd:?}"
        );

        let mut r#type = None;
        let mut prompt = None;
        let mut help = None;
        let mut defaults = Vec::new();
        let mut env = None;
        let mut depends_on = parent_condition.clone();
        let mut selects = Vec::new();
        let mut implies = Vec::new();
        let mut ranges = Vec::new();
        let mut comments = Vec::new();

        loop {
            let Some(tokens) = lines.peek() else {
                break;
            };

            let Some(cmd) = tokens.peek() else {
                panic!("Expected config entry");
            };

            match cmd.token {
                Token::Choice
                | Token::Config
                | Token::EndChoice
                | Token::EndIf
                | Token::EndMenu
                | Token::If
                | Token::Mainmenu
                | Token::Menu
                | Token::MenuConfig
                | Token::ORSource
                | Token::OSource
                | Token::RSource
                | Token::Source => {
                    // Next block entry; stop here.
                    break;
                }

                Token::Bool | Token::Hex | Token::Int | Token::String | Token::Tristate => {
                    let mut tokens = lines.next().unwrap();
                    let type_token = tokens.next().unwrap();

                    r#type = Some(type_token.r#type().unwrap());

                    if !tokens.is_empty() {
                        prompt = Some(Prompt::parse(type_token.location(), &mut tokens)?);
                    }
                }

                Token::Comment => {
                    let mut tokens = lines.next().unwrap();
                    let (cmd, comment) = tokens.read_cmd_str_lit(true)?;
                    assert_eq!(cmd.token, Token::Comment);
                    comments.push(comment);
                }

                Token::Default => {
                    let mut tokens = lines.next().unwrap();
                    let default = ConfigDefault::parse(&mut tokens)?;

                    defaults.push(default);
                }

                Token::Depends => {
                    let mut tokens = lines.next().unwrap();
                    let depends = Expr::parse_depends_on(&mut tokens)?;
                    depends_on = Expr::and(depends_on, depends);
                }

                Token::Prompt => {
                    let mut tokens = lines.next().unwrap();
                    _ = tokens.next();
                    assert!(tokens.peek().is_some());
                    prompt = Some(Prompt::parse(cmd.location(), &mut tokens)?);
                }

                Token::Help => {
                    let mut tokens = lines.next().unwrap();
                    help = Some(tokens.read_help()?);
                }

                Token::Imply => {
                    let mut tokens = lines.next().unwrap();
                    let config_target = ConfigTarget::parse(&mut tokens)?;
                    implies.push(config_target);
                }

                Token::Select => {
                    let mut tokens = lines.next().unwrap();
                    let config_target = ConfigTarget::parse(&mut tokens)?;
                    selects.push(config_target);
                }

                Token::Range => {
                    let mut tokens = lines.next().unwrap();
                    let range = ConfigRange::parse(&mut tokens)?;
                    ranges.push(range);
                }

                Token::Option => {
                    let mut tokens = lines.next().unwrap();
                    env = Some(Self::parse_option(&mut tokens)?);
                }

                _ => todo!("Not implemened: {cmd}"),
            }
        }

        let r#type = r#type.unwrap_or(Type::Unknown);

        let config = Self {
            name: name.clone(),
            r#type,
            prompt,
            defaults,
            env,
            depends_on,
            selects,
            implies,
            ranges,
            help,
            comments,
            parent,
        };

        // If there's an existing config with this name, replace it.
        if let Some(old_id) = kconfig.configs.get(config.name.as_ref()) {
            let old = kconfig.blocks.remove(*old_id).unwrap().into_config_or_menuconfig().unwrap();
            warn!("Redefining config {} at {}; previous definition is at {}", config.name, config.name.location(), old.name.location());
        }
        
        let block = match blk_cmd.token {
            Token::Config => Block::Config(config),
            Token::MenuConfig => Block::MenuConfig(config),
            _ => unreachable!(),
        };

        let block_id = kconfig.blocks.insert(block);
        kconfig.configs.insert(name.into_inner(), block_id);
        Ok(block_id)
    }

    fn parse_option(tokens: &mut TokenLine) -> Result<LocString, KConfigError> {
        let Some(cmd) = tokens.next() else {
            panic!("Expected option command");
        };

        let Some(env_token) = tokens.next() else {
            return Err(KConfigError::missing(Expected::Env, cmd.location()));
        };

        if env_token.token != Token::Env {
            return Err(KConfigError::unexpected(env_token, Expected::Env, env_token.location()));
        }

        let Some(eq_token) = tokens.next() else {
            return Err(KConfigError::missing(Expected::Eq, env_token.location()));
        };

        if eq_token.token != Token::Eq {
            return Err(KConfigError::unexpected(eq_token, Expected::Eq, eq_token.location()));
        }

        let Some(env_name) = tokens.next() else {
            return Err(KConfigError::missing(Expected::StringLiteral, eq_token.location()));
        };

        let Some(env_name) = env_name.string_literal_value() else {
            return Err(KConfigError::unexpected(env_name, Expected::StringLiteral, env_name.location()));
        };

        if let Some(unexpected) = tokens.next() {
            return Err(KConfigError::unexpected(unexpected, Expected::Eol, unexpected.location()));
        }

        Ok(env_name.to_loc_string())
    }
}

impl ConfigDefault {
    /// Parse the remainder of `default` statement within a config block (everything after the `default` keyword).
    pub fn parse(tokens: &mut TokenLine) -> Result<Self, KConfigError> {
        let Some(default_cmd) = tokens.next() else {
            panic!("Expected default command");
        };

        let value = Expr::parse(default_cmd.location(), tokens)?;

        let condition = if let Some(if_token) = tokens.next() {
            if if_token.token != Token::If {
                return Err(KConfigError::unexpected(if_token, Expected::IfOrEol, if_token.location()));
            }

            let cond = Expr::parse(if_token.location(), tokens)?;

            if let Some(unexpected) = tokens.next() {
                return Err(KConfigError::unexpected(unexpected, Expected::Eol, unexpected.location()));
            }

            cond
        } else {
            Expr::Tristate(Tristate::True)
        };

        Ok(Self {
            value,
            condition,
        })
    }
}

impl ConfigTarget {
    /// Parse the remainder of a `select` or `imply` statement (after the `select` or `imply` keyword).
    pub fn parse(tokens: &mut TokenLine) -> Result<Self, KConfigError> {
        let (cmd, target_name) = tokens.read_cmd_sym(false)?;
        assert!(matches!(cmd.token, Token::Select | Token::Imply));

        let condition = tokens.read_if_expr(true)?.unwrap_or(Expr::Tristate(Tristate::True));

        Ok(Self {
            target_name: target_name.to_string(),
            condition,
        })
    }
}

impl ConfigRange {
    /// Parse the remainder of a range statement (after the `range` keyword).
    pub fn parse(tokens: &mut TokenLine) -> Result<Self, KConfigError> {
        let Some(range_token) = tokens.next() else {
            panic!("Expected range command");
        };

        let Some(start) = tokens.next() else {
            return Err(KConfigError::missing(Expected::LitValue, range_token.location()));
        };

        let Some(start) = start.literal_value() else {
            return Err(KConfigError::unexpected(start, Expected::LitValue, start.location()));
        };

        let Some(end) = tokens.next() else {
            return Err(KConfigError::missing(Expected::LitValue, range_token.location()));
        };

        let Some(end) = end.literal_value() else {
            return Err(KConfigError::unexpected(end, Expected::LitValue, end.location()));
        };

        let condition = if let Some(if_token) = tokens.next() {
            if if_token.token != Token::If {
                return Err(KConfigError::unexpected(if_token, Expected::IfOrEol, if_token.location()));
            }

            Expr::parse(if_token.location(), tokens)?
        } else {
            Expr::Tristate(Tristate::True)
        };

        if let Some(unexpected) = tokens.next() {
            return Err(KConfigError::unexpected(unexpected, Expected::Eol, unexpected.location()));
        }

        Ok(Self {
            start,
            end,
            condition,
        })
    }
}
