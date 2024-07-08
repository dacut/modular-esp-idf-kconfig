use {
    crate::{
        parser::{
            Block, BlockId, Config, Expected, Expr, GetLocation, KConfig, KConfigError, LocString, PeekableTokenLines,
            Prompt, Token, TokenLine, Tristate,
        },
        Context,
    },
    std::{collections::HashSet, path::Path},
};

/// Choice entry.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Choice {
    /// The name of the choice.
    pub name: LocString,

    /// Optional prompt for the choice.
    pub prompt: Option<Prompt>,

    /// Optional help text for the choice.
    pub help: Option<LocString>,

    /// Possible symbols for the choice, represented as [`Config`] entries.
    pub configs: Vec<BlockId>,

    /// Default values for the choice.
    pub defaults: Vec<ChoiceDefault>,

    /// Dependencies for this config from `depend on` statements.
    pub depends_on: Expr,

    /// The parent menu (if any) of this choice.
    pub parent: Option<BlockId>,
}

/// A possible default for a choice entry.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ChoiceDefault {
    /// The target to choose for this default.
    pub target: LocString,

    /// A condition for this default. If unspecified in the KConfig, this is `Expr::Tristate(Tristate::True)`
    pub condition: Expr,
}

impl Choice {
    /// Parse a choice block.
    pub fn parse<C: Context>(
        kconfig: &mut KConfig,
        lines: &mut PeekableTokenLines,
        base_dir: &Path,
        parent_condition: Expr,
        parent: Option<BlockId>,
        context: &C,
    ) -> Result<BlockId, KConfigError> {
        let Some(mut tokens) = lines.next() else {
            panic!("Expected choice block");
        };

        let (blk_cmd, name) = tokens.read_cmd_sym(true)?;
        assert_eq!(blk_cmd.token, Token::Choice);

        let mut last_loc = name.get_location();

        let choice = Self {
            name,
            prompt: None,
            help: None,
            configs: Vec::new(),
            defaults: Vec::new(),
            depends_on: parent_condition.clone(),
            parent,
        };

        let block_id = kconfig.blocks.insert(Block::Choice(choice));
        let mut prompt = None;
        let mut configs = vec![];
        let mut depends_on = parent_condition.clone();
        let mut defaults = vec![];
        let mut help = None;

        loop {
            let Some(tokens) = lines.peek() else {
                return Err(KConfigError::unexpected_eof(Expected::EndChoice, last_loc));
            };

            let Some(cmd) = tokens.peek() else {
                panic!("Expected choice entry");
            };

            last_loc = cmd.get_location();

            match cmd.token {
                Token::EndChoice => {
                    _ = lines.next();
                    break;
                }

                Token::Config => {
                    let config =
                        Config::parse(kconfig, lines, base_dir, parent_condition.clone(), Some(block_id), context)?;
                    configs.push(config);
                }

                Token::Default => {
                    let mut tokens = lines.next().unwrap();
                    let default = ChoiceDefault::parse(&mut tokens)?;
                    defaults.push(default);
                }

                Token::Depends => {
                    let mut tokens = lines.next().unwrap();
                    let dependency = Expr::parse_depends_on(&mut tokens)?;
                    depends_on = Expr::and(depends_on, dependency);
                }

                Token::Help => {
                    let mut tokens = lines.next().unwrap();
                    help = Some(tokens.read_help()?);
                }

                // In some cases in ESP-IDF (components/bootloader/Kconfig.projbuild), the prompt is erroneously
                // specified for the choice as `bool "prompt"`. We handle it here to avoid a parse error.
                Token::Prompt | Token::Bool => {
                    let mut tokens = lines.next().unwrap();
                    let cmd = tokens.next().unwrap();
                    prompt = Some(Prompt::parse(cmd.get_location(), &mut tokens)?);
                }

                _ => unimplemented!("Choice entry not handled: {cmd:?}"),
            }
        }

        // Modify the fields we just read.
        let choice = kconfig.blocks.get_mut(block_id).unwrap().as_choice_mut().unwrap();
        choice.prompt = prompt;
        choice.configs = configs;
        choice.defaults = defaults;
        choice.depends_on = depends_on;
        choice.help = help;

        Ok(block_id)
    }

    /// Return all of the symbols this choice depends on.
    pub fn ref_symbols(&self) -> HashSet<String> {
        let mut result = HashSet::default();
        for default in self.defaults.iter() {
            default.cond_symbols_into(&mut result);
        }

        self.depends_on.symbols_into(&mut result);
        result
    }
}

impl ChoiceDefault {
    /// Parse the remainder of a `default` line within a choice block.
    pub fn parse(tokens: &mut TokenLine) -> Result<Self, KConfigError> {
        let (cmd, target) = tokens.read_cmd_sym(false)?;

        assert!(cmd.token == Token::Default);

        let condition = if let Some(if_token) = tokens.next() {
            if if_token.token != Token::If {
                return Err(KConfigError::unexpected(if_token, Expected::IfOrEol, if_token.get_location()));
            }

            let cond = Expr::parse(if_token.get_location(), tokens)?;

            if let Some(unexpected) = tokens.next() {
                return Err(KConfigError::unexpected(unexpected, Expected::Eol, unexpected.get_location()));
            }

            cond
        } else {
            Expr::Tristate(Tristate::True)
        };

        Ok(Self {
            target,
            condition,
        })
    }

    /// Return all of the symbols for the condition.
    pub(crate) fn cond_symbols_into(&self, result: &mut HashSet<String>) {
        self.condition.symbols_into(result)
    }
}
