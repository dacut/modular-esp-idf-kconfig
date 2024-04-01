use {
    crate::{
        parser::{
            Choice, Config, Expected, Expr, KConfigError, LocExpr, LocString, Located, Menu, PeekableTokenLines,
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

/// A conditional inclusion block.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IfBlock {
    /// The condition for the block.
    pub condition: LocExpr,

    /// The items in the block.
    pub items: Vec<Rc<RefCell<Block>>>,
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

                    items.push(Rc::new(RefCell::new(block)));
                }
            }
        }

        Ok(Self {
            condition,
            items,
        })
    }
}

impl ResolveBlock for IfBlock {
    type Output = Vec<Rc<RefCell<Block>>>;

    fn resolve_block<C>(&self, base_dir: &Path, context: &C, parent_cond: Option<&LocExpr>) -> Result<Self::Output, KConfigError>
    where
        C: Context,
    {
        let mut result = Vec::with_capacity(self.items.len());

        // AND the parent condition with the current condition.
        let sub_cond = if let Some(parent_cond) = parent_cond {
            let sub_expr = Expr::And(Box::new(parent_cond.clone()), Box::new(self.condition.clone()));
            LocExpr::new(sub_expr, self.condition.location())
        } else {
            self.condition.clone()
        };

        for item in self.items.iter() {
            let items = item.resolve_block(base_dir, context, Some(&sub_cond))?;
            result.extend(items);
        }

        Ok(result)
    }
}
