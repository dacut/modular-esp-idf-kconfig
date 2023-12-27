use {
    phf::phf_map,
    std::{
        backtrace::Backtrace,
        error::Error,
        fmt::{Display, Formatter, Result as FmtResult},
        str::FromStr,
    },
};

/// Tokens for the Kconfig language, in the same order as in kconfiglib.
#[allow(missing_docs)]
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[repr(u8)]
pub enum Token {
    AllNoConfigY = 1,
    And,
    Bool,
    Choice,
    CloseParen,
    Comment,
    Config,
    Default,
    DefConfigList,
    DefBool,
    DefHex,
    DefInt,
    DefString,
    DefTristate,
    Depends,
    EndChoice,
    EndIf,
    EndMenu,
    Env,
    Eq,
    Gt,
    Ge,
    Help,
    Hex,
    If,
    Imply,
    Int,
    Lt,
    Le,
    MainMenu,
    Menu,
    MenuConfig,
    Modules,
    Not,
    On,
    OpenParen,
    Option,
    Optional,
    Or,
    ORSource,
    OSource,
    Prompt,
    Range,
    RSource,
    Select,
    Source,
    String,
    Tristate,
    Unequal,
    Visible,
}

impl Token {
    /// Indicates whether a string is expected after this token. This is used to tell strings from constant symbol
    /// references durng tokenization, both of which are enclosed in quotes.
    pub fn expects_string(self) -> bool {
        matches!(
            self,
            Self::Bool
                | Self::Choice
                | Self::Comment
                | Self::Hex
                | Self::Int
                | Self::MainMenu
                | Self::Menu
                | Self::ORSource
                | Self::OSource
                | Self::Prompt
                | Self::RSource
                | Self::Source
                | Self::String
                | Self::Tristate
        )
    }

    /// Indicates whether this is a type token.
    pub fn is_type_token(self) -> bool {
        matches!(self, Self::Bool | Self::Int | Self::Hex | Self::String | Self::Tristate)
    }

    /// Indicates whether this is a source token.
    pub fn is_source_token(self) -> bool {
        matches!(self, Self::ORSource | Self::OSource | Self::RSource | Self::Source)
    }

    /// Indicates whether this is a relative source token.
    pub fn is_relative_source_token(self) -> bool {
        matches!(self, Self::ORSource | Self::RSource)
    }

    /// Indicates whether this is a required source token.
    pub fn is_required_source_token(self) -> bool {
        matches!(self, Self::RSource | Self::Source)
    }

    /// Indicates whether this is a relation (comparison) token.
    pub fn is_relation_token(self) -> bool {
        matches!(self, Self::Eq | Self::Gt | Self::Ge | Self::Lt | Self::Le | Self::Unequal)
    }
}

/// Return a token for the given string.
static KEYWORDS: phf::Map<&'static str, Token> = phf_map! {
    "---help---" => Token::Help,
    "allnoconfig_y" => Token::AllNoConfigY,
    "bool" => Token::Bool,
    "boolean" => Token::Bool,
    "choice" => Token::Choice,
    "comment" => Token::Comment,
    "config" => Token::Config,
    "def_bool" => Token::DefBool,
    "def_hex" => Token::DefHex,
    "def_int" => Token::DefInt,
    "def_string" => Token::DefString,
    "def_tristate" => Token::DefTristate,
    "default" => Token::Default,
    "defconfig_list" => Token::DefConfigList,
    "depends" => Token::Depends,
    "endchoice" => Token::EndChoice,
    "endif" => Token::EndIf,
    "endmenu" => Token::EndMenu,
    "env" => Token::Env,
    "grsource" => Token::ORSource,
    "gsource" => Token::OSource,
    "help" => Token::Help,
    "hex" => Token::Hex,
    "if" => Token::If,
    "imply" => Token::Imply,
    "int" => Token::Int,
    "mainmenu" => Token::MainMenu,
    "menu" => Token::Menu,
    "menuconfig" => Token::MenuConfig,
    "modules" => Token::Modules,
    "on" => Token::On,
    "option" => Token::Option,
    "optional" => Token::Optional,
    "orsource" => Token::ORSource,
    "osource" => Token::OSource,
    "prompt" => Token::Prompt,
    "range" => Token::Range,
    "rsource" => Token::RSource,
    "select" => Token::Select,
    "source" => Token::Source,
    "string" => Token::String,
    "tristate" => Token::Tristate,
    "visible" => Token::Visible,
};

impl FromStr for Token {
    type Err = UnknownTokenError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        KEYWORDS.get(s).copied().ok_or_else(|| UnknownTokenError::new(s.to_string()))
    }
}

#[derive(Debug)]
pub struct UnknownTokenError {
    token: String,
    backtrace: Backtrace,
}

impl UnknownTokenError {
    pub fn new(token: String) -> Self {
        Self {
            token,
            backtrace: Backtrace::capture(),
        }
    }

    pub fn token(&self) -> &str {
        &self.token
    }

    pub fn backtrace(&self) -> &Backtrace {
        &self.backtrace
    }
}

impl Display for UnknownTokenError {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        write!(f, "Unknown token: {}", self.token)
    }
}

impl Error for UnknownTokenError {}
