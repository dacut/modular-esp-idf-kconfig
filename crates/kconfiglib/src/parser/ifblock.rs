use {
    crate::{
        parser::{Block, Expected, Expr, KConfigError, LocExpr, Located, PeekableTokenLines, Token},
        Context, ResolveBlock,
    },
    std::{cell::RefCell, path::Path, rc::Rc},
};

/// A conditional inclusion block.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IfBlock {
    /// The condition for the block.
    pub condition: LocExpr,

    /// The items in the block.
    pub items: Vec<Rc<RefCell<Block>>>,
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

    fn resolve_block<C>(
        &self,
        base_dir: &Path,
        context: &C,
        parent_cond: Option<&LocExpr>,
    ) -> Result<Self::Output, KConfigError>
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
