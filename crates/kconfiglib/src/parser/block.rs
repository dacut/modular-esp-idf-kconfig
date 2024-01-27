use {
    crate::parser::{
        Choice, Config, Context, Expected, Expr, KConfigError, Located, Menu, PeekableTokenLines, Source, Token,
        TokenLine,
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
    Mainmenu(Located<String>),

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
    pub condition: Located<Expr>,

    /// The items in the block.
    pub items: Vec<Located<Block>>,
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
    pub fn parse(lines: &mut PeekableTokenLines, base_dir: &Path) -> Result<Option<Located<Block>>, KConfigError> {
        let Some(tokens) = lines.peek() else {
            return Ok(None);
        };

        let Some(cmd) = tokens.peek() else {
            panic!("Expected block command");
        };

        match cmd.as_ref() {
            Token::Choice => {
                let choice = Choice::parse(lines)?;
                Ok(Some(Located::new(Block::Choice(choice), cmd.location().clone())))
            }

            Token::Config => {
                let config = Config::parse(lines)?;
                Ok(Some(Located::new(Block::Config(config), cmd.location().clone())))
            }

            Token::If => {
                let if_block = IfBlock::parse(lines, base_dir)?;
                Ok(Some(Located::new(Block::If(if_block), cmd.location().clone())))
            }

            Token::MenuConfig => {
                let config = Config::parse(lines)?;
                Ok(Some(Located::new(Block::MenuConfig(config), cmd.location().clone())))
            }

            Token::Mainmenu => {
                let mut tokens = lines.next().unwrap();
                let main_menu = Self::parse_mainmenu(&mut tokens)?;
                Ok(Some(Located::new(Block::Mainmenu(main_menu), cmd.location().clone())))
            }

            Token::Menu => {
                let menu = Menu::parse(lines, base_dir)?;
                Ok(Some(Located::new(Block::Menu(menu), cmd.location().clone())))
            }

            Token::Source | Token::OSource | Token::RSource | Token::ORSource => {
                let mut tokens = lines.next().unwrap();
                let source = Source::parse(&mut tokens, base_dir)?;
                Ok(Some(Located::new(Block::Source(source), cmd.location().clone())))
            }

            _ => todo!("Block not handled: {cmd:?}"),
        }
    }

    fn parse_mainmenu(tokens: &mut TokenLine) -> Result<Located<String>, KConfigError> {
        let (cmd, title) = tokens.read_cmd_str_lit(true)?;
        assert!(matches!(cmd.as_ref(), Token::Mainmenu));
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

impl LocatedBlocks for Vec<Located<Block>> {
    fn resolve_blocks_recursive<C>(&mut self, base_dir: &Path, context: &C) -> Result<(), KConfigError>
    where
        C: Context,
    {
        // Change this to extract_if() when https://github.com/rust-lang/rust/issues/43244 is complete.
        let mut i = 0;

        while i < self.len() {
            // Can't use a match block here since it will hold onto self[i] and we're removing it.
            if matches!(self[i].as_ref(), Block::Source(_)) {
                // Evaluate the source block.
                let block = self.remove(i);
                let Block::Source(ref s) = block.as_ref() else {
                    unreachable!();
                };

                let blocks = s.evaluate(base_dir, context)?;
                self.extend(blocks);
            } else if matches!(self[i].as_ref(), Block::If(_)) {
                // Evaluate the if block.
                let block = self.remove(i);
                let Block::If(i_blk) = block.into_element() else {
                    unreachable!();
                };

                let blocks = i_blk.evaluate()?;
                self.extend(blocks);
            } else {
                self[i].as_mut().resolve_blocks_recursive(base_dir, context)?;
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
        assert!(matches!(if_token.as_ref(), Token::If));

        let condition = Expr::parse(if_token.location(), &mut tokens)?;

        if let Some(unexpected) = tokens.next() {
            return Err(KConfigError::unexpected(unexpected, Expected::Eol, unexpected.location()));
        }

        let mut items = Vec::new();
        let mut last_loc = condition.location().clone();

        loop {
            let Some(tokens) = lines.peek() else {
                return Err(KConfigError::unexpected_eof(Expected::EndIf, &last_loc));
            };

            let Some(cmd) = tokens.peek() else {
                panic!("Expected if entry");
            };

            last_loc = cmd.location().clone();

            match cmd.as_ref() {
                Token::EndIf => {
                    lines.next();
                    break;
                }
                _ => {
                    let Some(block) = Block::parse(lines, base_dir)? else {
                        return Err(KConfigError::unexpected_eof(Expected::EndIf, &last_loc));
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

    fn evaluate(self) -> Result<Vec<Located<Block>>, KConfigError> {
        let mut items = Vec::with_capacity(self.items.len());

        for mut item in self.items.into_iter() {
            if let Block::If(_) = item.as_ref() {
                let (if_blk, loc) = item.into_parts();
                let Block::If(mut if_blk) = if_blk else {
                    unreachable!();
                };

                // Box the conditions so they can be in an AND expression.
                let cond_a = self.condition.map(|e| Box::new(e.clone()));
                let cond_b = if_blk.condition.map(|e| Box::new(e.clone()));

                // Add our condition as an AND with the sub if-block's condition.
                if_blk.condition = Located::new(Expr::And(cond_a, cond_b), loc);

                // Then evaluate this sub-if block and append the results to our items.
                let sub_items = if_blk.evaluate()?;
                items.extend(sub_items);
            } else {
                match item.as_mut() {
                    Block::Choice(c) => c.depends_on.push(self.condition.clone()),
                    Block::Config(c) => c.depends_on.push(self.condition.clone()),
                    Block::Menu(m) => m.depends_on.push(self.condition.clone()),
                    Block::MenuConfig(mc) => mc.depends_on.push(self.condition.clone()),
                    _ => (),
                }

                items.push(item);
            }
        }

        Ok(items)
    }
}
