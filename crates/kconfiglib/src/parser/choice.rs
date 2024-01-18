use crate::parser::{Config, Expected, Expr, KConfigError, Located, PeekableTokenLines, Prompt, Token, TokenLine};

/// Choice entry.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Choice {
    /// The name of the choice.
    pub name: Located<String>,

    /// Optional prompt for the choice.
    pub prompt: Option<Prompt>,

    /// Optional help text for the choice.
    pub help: Option<Located<String>>,

    /// Possible symbols for the choice, represented as [`Config`] entries.
    pub configs: Vec<Config>,

    /// Default values for the choice.
    pub defaults: Vec<ChoiceDefault>,

    /// Dependencies for this config from `depend on` statements.
    pub depends_on: Vec<Located<Expr>>,
}

/// A possible default for a choice entry.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ChoiceDefault {
    /// The target to choose for this default.
    pub target: String,

    /// An optional condition for this default. If unspecified, this is equivalent to `y` (always true).
    pub condition: Option<Located<Expr>>,
}

impl Choice {
    /// Parse a choice block.
    pub fn parse(lines: &mut PeekableTokenLines) -> Result<Self, KConfigError> {
        let Some(mut tokens) = lines.next() else {
            panic!("Expected choice block");
        };

        let (blk_cmd, name) = tokens.read_cmd_sym(true)?;
        assert_eq!(blk_cmd.as_ref(), &Token::Choice);

        let mut prompt = None;
        let mut help = None;
        let mut configs = Vec::new();
        let mut defaults = Vec::new();
        let mut last_loc = name.location().clone();
        let mut depends_on = Vec::new();

        loop {
            let Some(tokens) = lines.peek() else {
                return Err(KConfigError::unexpected_eof(Expected::EndChoice, &last_loc));
            };

            let Some(cmd) = tokens.peek() else {
                panic!("Expected choice entry");
            };

            last_loc = cmd.location().clone();

            match cmd.as_ref() {
                Token::EndChoice => {
                    _ = lines.next();
                    break;
                }

                Token::Config => {
                    let config = Config::parse(lines)?;
                    configs.push(config);
                }

                Token::Default => {
                    let mut tokens = lines.next().unwrap();
                    let default = ChoiceDefault::parse(&mut tokens)?;
                    defaults.push(default);
                }

                Token::Depends => {
                    let mut tokens = lines.next().unwrap();
                    let depends = Expr::parse_depends_on(&mut tokens)?;
                    depends_on.push(depends);
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
                    prompt = Some(Prompt::parse(cmd.location(), &mut tokens)?);
                }

                _ => unimplemented!("Choice entry not handled: {cmd:?}"),
            }
        }

        let choice = Choice {
            name,
            prompt,
            help,
            configs,
            defaults,
            depends_on,
        };

        Ok(choice)
    }
}

impl ChoiceDefault {
    /// Parse the remainder of a `default` line within a choice block.
    pub fn parse(tokens: &mut TokenLine) -> Result<Self, KConfigError> {
        let (cmd, target) = tokens.read_cmd_sym(false)?;

        assert!(cmd.as_ref() == &Token::Default);

        let condition = if let Some(if_token) = tokens.next() {
            if if_token.as_ref() != &Token::If {
                return Err(KConfigError::unexpected(if_token, Expected::IfOrEol, if_token.location()));
            }

            let cond = Expr::parse(if_token.location(), tokens)?;

            if let Some(unexpected) = tokens.next() {
                return Err(KConfigError::unexpected(unexpected, Expected::Eol, unexpected.location()));
            }

            Some(cond)
        } else {
            None
        };

        let target = target.to_string();

        Ok(Self {
            target,
            condition,
        })
    }
}
