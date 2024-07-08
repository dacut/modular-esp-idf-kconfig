use {
    crate::{
        parser::{
            Block, BlockId, Expected, Expr, GetLocation, KConfig, KConfigError, LocString, PeekableTokenLines, Token,
            Tristate,
        },
        Context,
    },
    std::path::Path,
};

/// A menu block in a Kconfig file.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Menu {
    /// The prompt for the menu.
    pub prompt: LocString,

    /// The items in the menu.
    pub blocks: Vec<BlockId>,

    /// Dependencies for this menu from `depend on` statements.
    pub depends_on: Expr,

    /// Visibility in the menu. If not set in the KConfig, this is `Expr::Tristate(Tristate::True)`.
    pub visibility: Expr,

    /// Comments for the menu.
    pub comments: Vec<LocString>,

    /// The parent menu (if any) of this menu.
    pub parent: Option<BlockId>,
}

impl Menu {
    /// Parse a menu block.
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
        base_dir: &Path,
        parent_condition: Expr,
        parent: Option<BlockId>,
        context: &C,
    ) -> Result<BlockId, KConfigError> {
        let mut tokens = lines.next().unwrap();
        assert!(!tokens.is_empty());

        let Some(blk_cmd) = tokens.next() else {
            panic!("Expected menu command");
        };
        assert_eq!(blk_cmd.token, Token::Menu);

        let Some(prompt) = tokens.next() else {
            return Err(KConfigError::missing(Expected::StringLiteral, blk_cmd.get_location()));
        };

        let Some(prompt) = prompt.string_literal_value() else {
            return Err(KConfigError::unexpected(prompt, Expected::Symbol, prompt.get_location()));
        };

        if let Some(unexpected) = tokens.next() {
            return Err(KConfigError::unexpected(unexpected, Expected::Eol, unexpected.get_location()));
        }

        let mut visibility = Expr::Tristate(Tristate::True);

        let menu = Self {
            prompt: prompt.to_loc_string(),
            blocks: Vec::new(),
            depends_on: Expr::Tristate(Tristate::True),
            visibility: visibility.clone(),
            comments: Vec::new(),
            parent,
        };
        let block_id = kconfig.blocks.insert(Block::Menu(menu));

        let mut last_loc = prompt.get_location();
        let mut items = Vec::new();
        let mut depends_on = Expr::Tristate(Tristate::True);
        let mut comments = Vec::new();

        loop {
            let Some(tokens) = lines.peek() else {
                return Err(KConfigError::unexpected_eof(Expected::EndMenu, last_loc));
            };

            let Some(cmd) = tokens.peek() else {
                panic!("Expected menu entry");
            };

            if let Some(last_loc_known) = last_loc {
                if Some(last_loc_known) == cmd.get_location() {
                    panic!("No progress made in Menu::parse at {last_loc_known}")
                }
            }

            last_loc = cmd.get_location();

            match cmd.token {
                Token::EndMenu => {
                    _ = lines.next();
                    break;
                }

                Token::Comment => {
                    let mut tokens = lines.next().unwrap();
                    let (cmd, comment) = tokens.read_cmd_str_lit(true)?;
                    assert_eq!(cmd.token, Token::Comment);
                    comments.push(comment);
                }

                Token::Depends => {
                    let mut tokens = lines.next().unwrap();
                    let depends = Expr::parse_depends_on(&mut tokens)?;
                    depends_on = Expr::and(depends_on, depends);
                }

                Token::Visible => {
                    let mut tokens = lines.next().unwrap();
                    let vis = Expr::parse_visible_if(&mut tokens)?;
                    visibility = vis;
                }
                _ => {
                    let sub_block_ids = Block::parse_blocks(
                        kconfig,
                        lines,
                        base_dir,
                        parent_condition.clone(),
                        Some(block_id),
                        context,
                    )?;

                    items.extend(sub_block_ids);
                }
            }
        }

        // Update the menu with the parsed items.
        let menu = kconfig.blocks.get_mut(block_id).unwrap().as_menu_mut().unwrap();
        menu.blocks = items;
        menu.depends_on = depends_on;
        menu.visibility = visibility;
        menu.comments = comments;

        Ok(block_id)
    }
}
