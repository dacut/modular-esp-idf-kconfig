use {
    crate::{
        parser::{
            Choice, Config, IfBlock, KConfigError, LocExpr, LocString, Menu, PeekableTokenLines,
            Source, Token, TokenLine,
        },
        Context, ResolveBlock,
    },
    std::{cell::RefCell, path::Path, rc::Rc},
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

impl ResolveBlock for Rc<RefCell<Block>> {
    type Output = Vec<Rc<RefCell<Block>>>;

    fn resolve_block<C>(&self, base_dir: &Path, context: &C, parent_cond: Option<&LocExpr>) -> Result<Self::Output, KConfigError>
    where
        C: Context,
    {
        match &*self.borrow() {
            Block::If(ref i) => {
                let blocks = i.resolve_block(base_dir, context, parent_cond)?;
                for block in blocks.iter() {
                    if block.borrow().as_if().is_some() {
                        panic!("Expected if block to be resolved: {:?}", block.borrow());
                    }
                }
                Ok(blocks)
            }
            Block::Menu(ref m) => {
                let menu = m.resolve_block(base_dir, context, parent_cond)?;
                for block in menu.blocks.iter() {
                    if block.borrow().as_if().is_some() {
                        panic!("Expected if block to be resolved: {:?}", block.borrow());
                    }
                }
                Ok(vec![Rc::new(RefCell::new(Block::Menu(menu)))])
            }
            Block::Source(ref s) => {
                let blocks = s.resolve_block(base_dir, context, parent_cond)?;
                for block in blocks.iter() {
                    if block.borrow().as_if().is_some() {
                        panic!("Expected if block to be resolved: {:?}", block.borrow());
                    }
                }
                Ok(blocks)
            }
            _ => Ok(vec![self.clone()]),
        }
    }
}

impl ResolveBlock for [Rc<RefCell<Block>>] {
    type Output = Vec<Rc<RefCell<Block>>>;

    fn resolve_block<C>(&self, base_dir: &Path, context: &C, parent_cond: Option<&LocExpr>) -> Result<Self::Output, KConfigError>
    where
        C: Context,
    {
        // Create a new vec to hold the new blocks.
        let mut new_blocks = Vec::with_capacity(self.len());

        for block in self.iter() {
            let expanded = block.resolve_block(base_dir, context, parent_cond)?;
            for block in expanded.iter() {
                if block.borrow().as_if().is_some() {
                    panic!("Expected if block to be resolved: {:?}", block.borrow());
                }
            }
            new_blocks.extend(expanded);
        }

        Ok(new_blocks)
    }
}
