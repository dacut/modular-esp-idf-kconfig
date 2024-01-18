use {
    crate::parser::{Block, Context, Expected, Expr, KConfigError, Located, LocatedBlocks, PeekableTokenLines, Token},
    std::path::Path,
};

/// A menu block in a Kconfig file.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Menu {
    /// The prompt for the menu.
    pub prompt: Located<String>,

    /// The items in the menu.
    pub blocks: Vec<Located<Block>>,

    /// Dependencies for this config from `depend on` statements.
    pub depends_on: Vec<Located<Expr>>,

    /// Visibility in the menu. If `None`, the menu is visibile by default
    /// (equivalent to `y`/`true`).
    pub visibility: Option<Located<Expr>>,

    /// Comments for the menu.
    pub comments: Vec<Located<String>>,
}

impl Menu {
    /// Parse a menu block.
    ///
    /// * Parameters
    pub fn parse(lines: &mut PeekableTokenLines, base_dir: &Path) -> Result<Self, KConfigError> {
        let mut tokens = lines.next().unwrap();
        assert!(!tokens.is_empty());

        let Some(blk_cmd) = tokens.next() else {
            panic!("Expected menu command");
        };
        assert!(matches!(blk_cmd.as_ref(), Token::Menu));

        let Some(prompt) = tokens.next() else {
            return Err(KConfigError::missing(Expected::StringLiteral, blk_cmd.location()));
        };

        let Some(prompt) = prompt.map(Token::string_literal_value).transpose() else {
            return Err(KConfigError::unexpected(prompt, Expected::Symbol, prompt.location()));
        };

        if let Some(unexpected) = tokens.next() {
            return Err(KConfigError::unexpected(unexpected, Expected::Eol, unexpected.location()));
        }

        let prompt = prompt.map(ToString::to_string);
        let mut last_loc = prompt.location().clone();
        let mut items = Vec::new();
        let mut depends_on = Vec::new();
        let mut visibility = None;
        let mut comments = Vec::new();

        loop {
            let Some(tokens) = lines.peek() else {
                return Err(KConfigError::unexpected_eof(Expected::EndMenu, &last_loc));
            };

            let Some(cmd) = tokens.peek() else {
                panic!("Expected menu entry");
            };

            last_loc = cmd.location().clone();

            match cmd.as_ref() {
                Token::EndMenu => {
                    _ = lines.next();
                    break;
                }

                Token::Comment => {
                    let mut tokens = lines.next().unwrap();
                    let (cmd, comment) = tokens.read_cmd_str_lit(true)?;
                    assert_eq!(cmd.as_ref(), &Token::Comment);
                    comments.push(comment);
                }

                Token::Depends => {
                    let mut tokens = lines.next().unwrap();
                    let depends = Expr::parse_depends_on(&mut tokens)?;
                    depends_on.push(depends);
                }

                Token::Visible => {
                    let mut tokens = lines.next().unwrap();
                    let vis = Expr::parse_visible_if(&mut tokens)?;
                    visibility = Some(vis);
                }
                _ => {
                    let Some(block) = Block::parse(lines, base_dir)? else {
                        return Err(KConfigError::unexpected_eof(Expected::EndMenu, &last_loc));
                    };

                    items.push(block);
                }
            }
        }

        Ok(Self {
            prompt,
            blocks: items,
            depends_on,
            visibility,
            comments,
        })
    }
}

impl LocatedBlocks for Menu {
    fn resolve_blocks_recursive<C>(&mut self, base_dir: &Path, context: &C) -> Result<(), KConfigError>
    where
        C: Context,
    {
        self.blocks.resolve_blocks_recursive(base_dir, context)
    }
}