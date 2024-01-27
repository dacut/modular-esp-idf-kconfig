use crate::parser::{Expected, KConfigError, LocExpr, LocString, Located, Location, Token, TokenLine};

/// Prompt for a config or choice block along with an optional condition.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Prompt {
    /// The prompt title.
    pub title: LocString,

    /// Optional expression that determines whether the prompt is shown.
    pub condition: Option<LocExpr>,
}

impl Prompt {
    /// Create a prompt with the given title.
    pub fn new(title: LocString) -> Self {
        Self {
            title,
            condition: None,
        }
    }

    /// Parse the remainder of a prompt statement (everything after the `prompt` keyword or a type keyword).
    pub fn parse(prev: Location, tokens: &mut TokenLine) -> Result<Self, KConfigError> {
        let Some(title) = tokens.next() else {
            return Err(KConfigError::missing(Expected::StringLiteral, prev));
        };

        let Some(title) = title.string_literal_value() else {
            return Err(KConfigError::unexpected(title, Expected::StringLiteral, title.location()));
        };

        let title = title.to_loc_string();

        let condition = if let Some(if_token) = tokens.next() {
            if if_token.token != Token::If {
                return Err(KConfigError::unexpected(if_token, Expected::IfOrEol, if_token.location()));
            }

            Some(LocExpr::parse(if_token.location(), tokens)?)
        } else {
            None
        };

        Ok(Prompt {
            title,
            condition,
        })
    }
}
