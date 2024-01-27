use {
    crate::parser::{
        Expected, KConfigError, LitValue, LocLitValue, LocStr, Located, Location, PeekableChars, Tristate, Type,
    },
    phf::phf_map,
    std::fmt::{Display, Formatter, Result as FmtResult},
};

/// Tokens for the Kconfig language
#[allow(missing_docs)]
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum Token {
    HexLit(u64),
    IntLit(i64),
    StrLit(String),
    Symbol(String),

    Bool,
    Hex,
    Int,
    String,
    Tristate,

    DefBool,
    DefHex,
    DefInt,
    DefString,
    DefTristate,

    Choice,
    Comment,
    Config,
    DefConfigList,
    EndChoice,
    Help,
    Mainmenu,
    Menu,
    EndMenu,
    MenuConfig,
    Modules,
    Prompt,

    AllNoConfigY,
    Default,
    Depends,
    Env,
    Imply,
    Option,
    Optional,
    Range,
    Select,
    Visible,

    Source,
    RSource,
    OSource,
    ORSource,

    LParen,
    RParen,

    If,
    EndIf,
    On,

    Not,
    Ne,
    Eq,
    Ge,
    Gt,
    Le,
    Lt,
    And,
    Or,
}

/// A token with location information.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LocToken {
    /// The token.
    pub token: Token,

    /// The location of the token.
    pub location: Location,
}

impl Token {
    /// Indicates whether a string is expected after this token. This is used to tell strings from constant symbol
    /// references durng tokenization, both of which are enclosed in quotes.
    // pub fn expects_string(&self) -> bool {
    //     matches!(
    //         self,
    //         Self::Bool
    //             | Self::Choice
    //             | Self::Hex
    //             | Self::Int
    //             | Self::Mainmenu
    //             | Self::Menu
    //             | Self::ORSource
    //             | Self::OSource
    //             | Self::Prompt
    //             | Self::RSource
    //             | Self::Source
    //             | Self::String
    //             | Self::Tristate
    //     )
    // }

    /// Indicates whether this is a type token.
    #[inline(always)]
    pub fn is_type(&self) -> bool {
        matches!(self, Self::Bool | Self::Int | Self::Hex | Self::String | Self::Tristate)
    }

    /// Indicates whether this is a relative source token.
    #[inline(always)]
    pub fn is_relative_source(&self) -> bool {
        matches!(self, Self::ORSource | Self::RSource)
    }

    /// Indicates whether this is an optional source token.
    #[inline(always)]
    pub fn is_optional_source(&self) -> bool {
        matches!(self, Self::OSource | Self::ORSource)
    }

    /// Indicates whether this is a comparison token.
    #[inline(always)]
    pub fn is_cmp(&self) -> bool {
        matches!(self, Self::Eq | Self::Ne | Self::Gt | Self::Ge | Self::Lt | Self::Le)
    }

    /// Indicates whether this is a source token.
    #[inline(always)]
    pub fn is_source(&self) -> bool {
        matches!(self, Self::ORSource | Self::OSource | Self::RSource | Self::Source)
    }

    /// Returns the literal value of this token if it is a literal, or `None` otherwise.
    pub fn literal_value(&self) -> Option<LitValue> {
        match self {
            Self::HexLit(h) => Some(LitValue::Hex(*h)),
            Self::IntLit(i) => Some(LitValue::Int(*i)),
            Self::StrLit(s) => Some(LitValue::String(s.clone())),
            Self::Symbol(s) => match s.as_str() {
                "n" => Some(LitValue::Tristate(Tristate::False)),
                "m" => Some(LitValue::Tristate(Tristate::Maybe)),
                "y" => Some(LitValue::Tristate(Tristate::True)),
                _ => Some(LitValue::Symbol(s.clone())),
            },
            _ => None,
        }
    }

    /// Returns the symbol name or `None` if this isn't a symbol.
    pub fn symbol_value(&self) -> Option<&str> {
        match self {
            Self::Symbol(s) => Some(s),
            _ => None,
        }
    }

    /// Returns the string literal value or `None` if this isn't a string literal.
    pub fn string_literal_value(&self) -> Option<&str> {
        match self {
            Self::StrLit(s) => Some(s),
            _ => None,
        }
    }

    /// Returns the type value or `None` if this isn't a type.
    pub fn r#type(&self) -> Option<Type> {
        match self {
            Self::Bool => Some(Type::Bool),
            Self::Hex => Some(Type::Hex),
            Self::Int => Some(Type::Int),
            Self::String => Some(Type::String),
            Self::Tristate => Some(Type::Tristate),
            _ => None,
        }
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
    "mainmenu" => Token::Mainmenu,
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

impl Display for Token {
    fn fmt(&self, f: &mut Formatter) -> FmtResult {
        match self {
            Self::HexLit(h) => write!(f, "{h:#x}"),
            Self::IntLit(i) => write!(f, "{i}"),
            Self::StrLit(s) => write!(f, "{s:?}"),
            Self::Symbol(s) => f.write_str(s),

            Self::Bool => f.write_str("bool"),
            Self::Hex => f.write_str("hex"),
            Self::Int => f.write_str("int"),
            Self::String => f.write_str("string"),
            Self::Tristate => f.write_str("tristate"),

            Self::DefBool => f.write_str("def_bool"),
            Self::DefHex => f.write_str("def_hex"),
            Self::DefInt => f.write_str("def_int"),
            Self::DefString => f.write_str("def_string"),
            Self::DefTristate => f.write_str("def_tristate"),

            Self::Choice => f.write_str("choice"),
            Self::Comment => f.write_str("comment"),
            Self::Config => f.write_str("config"),
            Self::DefConfigList => f.write_str("defconfig_list"),
            Self::EndChoice => f.write_str("endchoice"),
            Self::Help => f.write_str("help"),
            Self::Mainmenu => f.write_str("mainmenu"),
            Self::Menu => f.write_str("menu"),
            Self::EndMenu => f.write_str("endmenu"),
            Self::MenuConfig => f.write_str("menuconfig"),
            Self::Modules => f.write_str("modules"),
            Self::Prompt => f.write_str("prompt"),

            Self::AllNoConfigY => f.write_str("allnoconfig_y"),
            Self::Default => f.write_str("default"),
            Self::Depends => f.write_str("depends"),
            Self::Env => f.write_str("env"),
            Self::Imply => f.write_str("imply"),
            Self::Option => f.write_str("option"),
            Self::Optional => f.write_str("optional"),
            Self::Range => f.write_str("range"),
            Self::Select => f.write_str("select"),
            Self::Visible => f.write_str("visible"),

            Self::Source => f.write_str("source"),
            Self::RSource => f.write_str("rsource"),
            Self::OSource => f.write_str("osource"),
            Self::ORSource => f.write_str("orsource"),

            Self::If => f.write_str("if"),
            Self::EndIf => f.write_str("endif"),
            Self::On => f.write_str("on"),

            Self::LParen => f.write_str("("),
            Self::RParen => f.write_str(")"),
            Self::Not => f.write_str("!"),
            Self::Ne => f.write_str("!="),
            Self::Eq => f.write_str("="),
            Self::Ge => f.write_str(">="),
            Self::Gt => f.write_str(">"),
            Self::Le => f.write_str("<="),
            Self::Lt => f.write_str("<"),
            Self::And => f.write_str("&&"),
            Self::Or => f.write_str("||"),
        }
    }
}

impl LocToken {
    // Note: We prefer these instead of implementing Deref<Target = Token> because we want to
    // annotate location information on some return values.

    /// Indicates whether the underlying token is a type token.
    #[inline(always)]
    pub fn is_type(&self) -> bool {
        self.token.is_type()
    }

    /// Indicates whether the underlying token is a relative source token.
    #[inline(always)]
    pub fn is_relative_source(&self) -> bool {
        self.token.is_relative_source()
    }

    /// Indicates whether the underlying token is an optional source token.
    #[inline(always)]
    pub fn is_optional_source(&self) -> bool {
        self.token.is_optional_source()
    }

    /// Indicates whether the underlying token is a relation (comparison) token.
    #[inline(always)]
    pub fn is_relation(&self) -> bool {
        self.token.is_cmp()
    }

    /// Indicates whether the underlying token is a source token.
    #[inline(always)]
    pub fn is_source(&self) -> bool {
        self.token.is_source()
    }

    /// Returns the literal value of this token if it is a literal, or `None` otherwise.
    #[inline(always)]
    pub fn literal_value(&self) -> Option<LocLitValue> {
        self.token.literal_value().map(|v| LocLitValue::new(v, self.location))
    }

    /// Returns the symbol name or `None` if this isn't a symbol.
    #[inline(always)]
    pub fn symbol_value(&self) -> Option<LocStr> {
        self.token.symbol_value().map(|s| LocStr::new(s, self.location))
    }

    /// Returns the string literal value or `None` if this isn't a string literal.
    #[inline(always)]
    pub fn string_literal_value(&self) -> Option<LocStr> {
        self.token.string_literal_value().map(|s| LocStr::new(s, self.location))
    }

    /// Returns the type value or `None` if this isn't a type.
    #[inline(always)]
    pub fn r#type(&self) -> Option<Type> {
        self.token.r#type()
    }
}

impl Located for LocToken {
    fn location(&self) -> Location {
        self.location
    }
}

impl LocToken {
    /// Create a new located token.
    pub fn new(token: Token, location: Location) -> Self {
        Self {
            token,
            location,
        }
    }
}

impl Display for LocToken {
    fn fmt(&self, f: &mut Formatter) -> FmtResult {
        Display::fmt(&self.token, f)
    }
}

pub(crate) fn parse_keyword_or_symbol(chars: &mut PeekableChars) -> Result<LocToken, KConfigError> {
    let start = chars.location();
    let mut ident = String::new();
    let Some(c) = chars.next() else {
        return Err(KConfigError::unexpected_eof(Expected::KeywordOrSymbol, start));
    };

    if !c.is_alphabetic() && c != '_' {
        return Err(KConfigError::unexpected(c, Expected::KeywordOrSymbol, start));
    }

    ident.push(c);

    loop {
        let Some(c) = chars.peek() else {
            break;
        };

        if c.is_alphanumeric() || c == '_' {
            ident.push(c);
            _ = chars.next();
        } else {
            break;
        }
    }

    let token = match KEYWORDS.get(&ident) {
        Some(kw) => kw.clone(),
        None => Token::Symbol(ident),
    };

    Ok(LocToken::new(token, start))
}
