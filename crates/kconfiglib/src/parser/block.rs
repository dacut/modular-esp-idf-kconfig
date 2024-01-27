use {
    crate::parser::{
        Choice, Config, Context, Expected, Expr, KConfigError, LocExpr, LocString, Located, Menu, PeekableTokenLines,
        Source, Token, TokenLine,
    },
    std::path::Path,
};

/// A block in a Kconfig file.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Block {
    /// Choice of configuration entries.
    Choice(Choice),

    /// Configuration entry for a symbol.
    Config(Config),

    /// Conditional inclusion of entries.
    If(IfBlock),

    /// Main menu title.
    Mainmenu(LocString),

    /// Menu block containing other items visible to the user in a submenu.
    Menu(Menu),

    /// Configuration entry for a symbol with an attached menu.
    MenuConfig(Config),

    /// Source another Kconfig file.
    Source(Source),
}

/// A conditional inclusion block.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IfBlock {
    /// The condition for the block.
    pub condition: LocExpr,

    /// The items in the block.
    pub items: Vec<Block>,
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

    /// If this is a config block, return a reference to it; otherwise, return `None`.
    #[inline(always)]
    pub fn as_config(&self) -> Option<&Config> {
        match self {
            Block::Config(c) => Some(c),
            _ => None,
        }
    }

    /// If this is an if block, return a reference to it; otherwise, return `None`.
    #[inline(always)]
    pub fn as_if(&self) -> Option<&IfBlock> {
        match self {
            Block::If(i) => Some(i),
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

    /// If this is a menuconfig block, return a reference to it; otherwise, return `None`.
    #[inline(always)]
    pub fn as_menuconfig(&self) -> Option<&Config> {
        match self {
            Block::MenuConfig(mc) => Some(mc),
            _ => None,
        }
    }

    /// If this is a source block, return a reference to it; otherwise, return `None`.
    #[inline(always)]
    pub fn as_source(&self) -> Option<&Source> {
        match self {
            Block::Source(s) => Some(s),
            _ => None,
        }
    }

    /// Parse the next block from the stream.   
    pub fn parse(lines: &mut PeekableTokenLines, base_dir: &Path) -> Result<Option<Block>, KConfigError> {
        let Some(tokens) = lines.peek() else {
            return Ok(None);
        };

        let Some(cmd) = tokens.peek() else {
            panic!("Expected block command");
        };

        match cmd.token {
            Token::Choice => {
                let choice = Choice::parse(lines)?;
                Ok(Some(Block::Choice(choice)))
            }

            Token::Config => {
                let config = Config::parse(lines)?;
                Ok(Some(Block::Config(config)))
            }

            Token::If => {
                let if_block = IfBlock::parse(lines, base_dir)?;
                Ok(Some(Block::If(if_block)))
            }

            Token::MenuConfig => {
                let config = Config::parse(lines)?;
                Ok(Some(Block::MenuConfig(config)))
            }

            Token::Mainmenu => {
                let mut tokens = lines.next().unwrap();
                let main_menu = Self::parse_mainmenu(&mut tokens)?;
                Ok(Some(Block::Mainmenu(main_menu)))
            }

            Token::Menu => {
                let menu = Menu::parse(lines, base_dir)?;
                Ok(Some(Block::Menu(menu)))
            }

            Token::Source | Token::OSource | Token::RSource | Token::ORSource => {
                let mut tokens = lines.next().unwrap();
                let source = Source::parse(&mut tokens, base_dir)?;
                Ok(Some(Block::Source(source)))
            }

            _ => todo!("Block not handled: {cmd:?}"),
        }
    }

    fn parse_mainmenu(tokens: &mut TokenLine) -> Result<LocString, KConfigError> {
        let (cmd, title) = tokens.read_cmd_str_lit(true)?;
        assert!(matches!(cmd.token, Token::Mainmenu));
        Ok(title)
    }
}

/// A trait for blocks that contain other blocks; used to resolve `source` commands and `if` blocks that encompass
/// other blocks.
pub trait LocatedBlocks {
    /// Resolve `source` commands and `if` blocks that encompass other blocks.
    fn resolve_blocks_recursive<C>(&mut self, base_dir: &Path, context: &C) -> Result<(), KConfigError>
    where
        C: Context;
}

impl LocatedBlocks for Block {
    fn resolve_blocks_recursive<C>(&mut self, base_dir: &Path, context: &C) -> Result<(), KConfigError>
    where
        C: Context,
    {
        if let Block::Menu(m) = self {
            m.resolve_blocks_recursive(base_dir, context)?;
        }

        Ok(())
    }
}

impl LocatedBlocks for Vec<Block> {
    fn resolve_blocks_recursive<C>(&mut self, base_dir: &Path, context: &C) -> Result<(), KConfigError>
    where
        C: Context,
    {
        // Change this to extract_if() when https://github.com/rust-lang/rust/issues/43244 is complete.
        let mut i = 0;

        while i < self.len() {
            // Can't use a match block here since it will hold onto self[i] and we're removing it.
            if matches!(self[i], Block::Source(_)) {
                // Evaluate the source block.
                let block = self.remove(i);
                let Block::Source(ref s) = block else {
                    unreachable!();
                };

                let blocks = s.evaluate(base_dir, context)?;
                self.extend(blocks);
            } else if matches!(self[i], Block::If(_)) {
                // Evaluate the if block.
                let block = self.remove(i);
                let Block::If(i_blk) = block else {
                    unreachable!();
                };

                let blocks = i_blk.evaluate()?;
                self.extend(blocks);
            } else {
                self[i].resolve_blocks_recursive(base_dir, context)?;
                i += 1;
            }
        }

        Ok(())
    }
}

impl IfBlock {
    /// Parse a conditional inclusion block.
    pub fn parse(lines: &mut PeekableTokenLines, base_dir: &Path) -> Result<Self, KConfigError> {
        let mut tokens = lines.next().unwrap();
        assert!(!tokens.is_empty());

        let Some(if_token) = tokens.next() else {
            panic!("Expected if command");
        };
        assert!(matches!(if_token.token, Token::If));

        let condition = LocExpr::parse(if_token.location(), &mut tokens)?;

        if let Some(unexpected) = tokens.next() {
            return Err(KConfigError::unexpected(unexpected, Expected::Eol, unexpected.location()));
        }

        let mut items = Vec::new();
        let mut last_loc = condition.location();

        loop {
            let Some(tokens) = lines.peek() else {
                return Err(KConfigError::unexpected_eof(Expected::EndIf, last_loc));
            };

            let Some(cmd) = tokens.peek() else {
                panic!("Expected if entry");
            };

            last_loc = cmd.location();

            match cmd.token {
                Token::EndIf => {
                    lines.next();
                    break;
                }
                _ => {
                    let Some(block) = Block::parse(lines, base_dir)? else {
                        return Err(KConfigError::unexpected_eof(Expected::EndIf, last_loc));
                    };

                    items.push(block);
                }
            }
        }

        Ok(Self {
            condition,
            items,
        })
    }

    fn evaluate(self) -> Result<Vec<Block>, KConfigError> {
        let mut items = Vec::with_capacity(self.items.len());

        for mut item in self.items.into_iter() {
            match item {
                Block::Choice(ref mut c) => {
                    c.depends_on.push(self.condition.clone());
                    items.push(item);
                }
                Block::Config(ref mut c) => {
                    c.depends_on.push(self.condition.clone());
                    items.push(item);
                }
                Block::If(mut if_blk) => {
                    let cond_a = Box::new(self.condition.clone());
                    let cond_b = Box::new(if_blk.condition);

                    // Add our condition as an AND with the sub if-block's condition.
                    if_blk.condition = LocExpr::new(Expr::And(cond_a, cond_b), self.condition.location());
                    let sub_items = if_blk.evaluate()?;
                    items.extend(sub_items);
                }
                Block::Menu(ref mut m) => {
                    m.depends_on.push(self.condition.clone());
                    items.push(item);
                }
                Block::MenuConfig(ref mut mc) => {
                    mc.depends_on.push(self.condition.clone());
                    items.push(item);
                }

                _ => (),
            }
        }

        Ok(items)
    }
}
