use {
    crate::parser::{
        cache_path, comment::parse_comment, integer::parse_integer_literal, string_literal::parse_string_literal,
        token::parse_keyword_or_symbol, whitespace::parse_hws0, Expected, KConfigError, LocExpr, LocString, LocToken,
        Located, Location, Token,
    },
    std::{iter::FusedIterator, ops::Deref, path::Path},
};

/// An iterator over a string slice from a file that returns characters and can peek at the next character.
///
/// This is more powerful than Peekable<Chars>:
/// * It can return the remainder of the string.
/// * It can peek at more than the next character.
/// * [`&str`][str] methods such as [`starts_with()`][str::starts_with()] can be used via [`Deref`][Deref].
/// * It can return the location of the current string.
#[derive(Clone, Debug)]
pub struct PeekableChars<'buf> {
    base: &'buf str,
    offset: usize,
    location: Location,
}

impl<'buf> PeekableChars<'buf> {
    /// Create a new PeekableChars from a string slice and filename.
    pub fn new(base: &'buf str, filename: &Path) -> Self {
        Self {
            base,
            offset: 0,
            location: Location {
                filename: cache_path(filename.to_owned()),
                line: 1,
                column: 1,
            },
        }
    }

    /// Returns the underlying string.
    #[inline(always)]
    pub fn base_str(&self) -> &'buf str {
        self.base
    }

    /// Returns the current offset in the string.
    #[inline(always)]
    pub fn offset(&self) -> usize {
        self.offset
    }

    /// Returns the remaining length, in bytes, of the string.
    #[inline(always)]
    pub fn len(&self) -> usize {
        self.base.len() - self.offset
    }

    /// Returns true if there are no more bytes to read.
    #[inline(always)]
    pub fn is_empty(&self) -> bool {
        self.offset >= self.base.len()
    }

    /// Returns the line and column number of the specified offset.
    pub fn position_of(&self, offset: usize) -> (usize, usize) {
        let mut line = 1;
        let mut col = 1;

        for c in self.base[..offset].chars() {
            if c == '\n' {
                line += 1;
                col = 1;
            } else {
                col += 1;
            }
        }

        (line, col)
    }

    /// Peek at the next character in the string.
    #[inline(always)]
    pub fn peek(&self) -> Option<char> {
        self.base[self.offset..].chars().next()
    }

    /// Peek at the nth character in the string.
    #[inline(always)]
    pub fn peek_at(&self, n: usize) -> Option<char> {
        self.base[self.offset..].chars().nth(n)
    }

    // /// Return the remainder of the underlying string.
    // #[inline(always)]
    // pub fn remainder(&self) -> &'a str {
    //     &self.base[self.offset..]
    // }

    /// Return the section of the string that has already been processed.
    // #[inline(always)]
    // pub fn processed(&self) -> &'a str {
    //     &self.base[..self.offset]
    // }

    /// Advances the offset by the given number of bytes.
    #[inline(always)]
    pub fn advance(&mut self, n: usize) {
        if n == 0 {
            return;
        }

        let chars = self.base[self.offset..].chars();
        let target = self.offset + n;

        if target > self.base.len() {
            panic!("{n} advances to {target}, which is past the end of the string");
        }

        for c in chars {
            self.offset += c.len_utf8();
            if self.offset == target {
                break;
            }

            if self.offset > target {
                panic!("{n} advances to {target}, which is not a char boundary");
            }

            if c == '\n' {
                self.location.line += 1;
                self.location.column = 1;
            } else {
                self.location.column += 1;
            }
        }

        assert_eq!(self.offset, target);
    }

    /// Read characters until the given predicate returns true or the end of the string is reached.
    pub fn read_until(&mut self, predicate: impl CharPredicate) -> &'buf str {
        let chars = self.base[self.offset..].chars();
        let start = self.offset;

        for c in chars {
            if predicate.matches(c) {
                break;
            }

            self.offset += c.len_utf8();
            if c == '\n' {
                self.location.line += 1;
                self.location.column = 1;
            } else {
                self.location.column += 1;
            }
        }

        &self.base[start..self.offset]
    }
}

impl Located for PeekableChars<'_> {
    fn location(&self) -> Location {
        self.location
    }
}

impl<'buf> Deref for PeekableChars<'buf> {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        &self.base[self.offset..]
    }
}

impl<'buf> Iterator for PeekableChars<'buf> {
    type Item = char;

    fn next(&mut self) -> Option<Self::Item> {
        match self.peek() {
            Some(c) => {
                self.offset += c.len_utf8();
                match c {
                    '\n' => {
                        self.location.line += 1;
                        self.location.column = 1;
                    }
                    _ => {
                        self.location.column += 1;
                    }
                }
                Some(c)
            }
            None => None,
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let max = self.base.len() - self.offset;
        let min = (max + 3) / 4;
        (min, Some(max))
    }
}

impl<'buf> FusedIterator for PeekableChars<'buf> {}

/// A trait for predicates that match characters.
pub trait CharPredicate {
    /// Returns true if the character matches the predicate.
    fn matches(&self, c: char) -> bool;
}

impl<F> CharPredicate for F
where
    F: Fn(char) -> bool,
{
    fn matches(&self, c: char) -> bool {
        self(c)
    }
}

impl CharPredicate for char {
    fn matches(&self, c: char) -> bool {
        *self == c
    }
}

/// An iterator over lines of tokens that can peek ahead at the next line without consuming it.
pub struct PeekableTokenLines<'buf> {
    base: &'buf [Vec<LocToken>],
    offset: usize,
}

impl<'buf> PeekableTokenLines<'buf> {
    /// Peek at the next line in the string.
    #[inline(always)]
    pub fn peek(&self) -> Option<TokenLine<'buf>> {
        if self.offset < self.base.len() {
            Some(TokenLine {
                base: &self.base[self.offset],
                offset: 0,
            })
        } else {
            None
        }
    }

    /// Peek at the nth character in the string.
    #[inline(always)]
    pub fn peek_at(&self, n: usize) -> Option<TokenLine<'buf>> {
        if self.offset + n < self.base.len() {
            Some(TokenLine {
                base: &self.base[self.offset + n],
                offset: 0,
            })
        } else {
            None
        }
    }

    /// Return the remainder of the lines.
    #[inline(always)]
    pub fn remainder(&self) -> &'buf [Vec<LocToken>] {
        &self.base[self.offset..]
    }

    /// Return the section of the string that has already been processed.
    #[inline(always)]
    pub fn processed(&self) -> &'buf [Vec<LocToken>] {
        &self.base[..self.offset]
    }

    /// Move the offset to the current position. After this,
    /// [`processed()`][PeekableTokenLines::processed] will return an empty slice.
    // #[inline(always)]
    // pub fn set_to_current(&mut self) {
    //     self.base = &self.base[self.offset..];
    //     self.offset = 0;
    // }

    /// Advances the offset by the given number of lines.
    #[inline(always)]
    pub fn advance(&mut self, n: usize) {
        self.offset += n;
        if self.offset > self.base.len() {
            self.offset = self.base.len();
        }
    }
}

impl<'buf> Iterator for PeekableTokenLines<'buf> {
    type Item = TokenLine<'buf>;

    fn next(&mut self) -> Option<Self::Item> {
        match self.peek() {
            Some(line) => {
                self.offset += 1;
                Some(line)
            }
            None => None,
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let n = self.base.len() - self.offset;
        (n, Some(n))
    }
}

impl<'buf> FusedIterator for PeekableTokenLines<'buf> {}

/// An extension trait for `&[Vec<Token>]` that provides `peek_lines()`.
pub trait PeekableTokenLinesExt {
    /// Return a [`PeekableTokenLines`] iterator over the slice.
    fn peek_lines(&self) -> PeekableTokenLines;
}

impl PeekableTokenLinesExt for [Vec<LocToken>] {
    fn peek_lines(&self) -> PeekableTokenLines {
        PeekableTokenLines {
            base: self,
            offset: 0,
        }
    }
}

/// An iterator over a single line of tokens that can peek ahead at the next token without consuming it.
#[derive(Debug)]
pub struct TokenLine<'buf> {
    base: &'buf [LocToken],
    offset: usize,
}

impl<'buf> TokenLine<'buf> {
    /// Create a new `TokenLine` from the given slice of tokens.
    pub fn new(base: &'buf [LocToken]) -> Self {
        Self {
            base,
            offset: 0,
        }
    }

    /// Returns the underlying line of tokens as a slice.
    #[inline(always)]
    pub fn line(&self) -> &'buf [LocToken] {
        self.base
    }

    /// Returns the current token offset in the line.
    #[inline(always)]
    pub fn offset(&self) -> usize {
        self.offset
    }

    /// Returns the remaining number of tokens to read in the line.
    #[inline(always)]
    pub fn len(&self) -> usize {
        self.base.len() - self.offset
    }

    /// Returns true if there are no more tokens to read.
    #[inline(always)]
    pub fn is_empty(&self) -> bool {
        self.offset >= self.base.len()
    }

    /// Peek at the next token in the line.
    #[inline(always)]
    pub fn peek(&self) -> Option<&'buf LocToken> {
        if self.offset < self.base.len() {
            Some(&self.base[self.offset])
        } else {
            None
        }
    }

    /// Peek at the nth token in the line.
    #[inline(always)]
    pub fn peek_at(&self, n: usize) -> Option<&'buf LocToken> {
        if self.offset + n < self.base.len() {
            Some(&self.base[self.offset + n])
        } else {
            None
        }
    }

    /// Read a command followed by a symbol from the line.
    pub fn read_cmd_sym(&mut self, require_eol: bool) -> Result<(&LocToken, LocString), KConfigError> {
        let Some(cmd) = self.next() else {
            panic!("Expected keyword");
        };

        let Some(name) = self.next() else {
            return Err(KConfigError::missing(Expected::Symbol, cmd.location()));
        };

        let Some(name) = name.symbol_value() else {
            return Err(KConfigError::unexpected(name, Expected::Symbol, name.location()));
        };

        if require_eol {
            if let Some(unexpected) = self.next() {
                return Err(KConfigError::unexpected(unexpected, Expected::Eol, unexpected.location()));
            }
        }

        let name = name.to_loc_string();

        Ok((cmd, name))
    }

    /// Read a command followed by a string literal from the line.
    pub fn read_cmd_str_lit(&mut self, require_eol: bool) -> Result<(&LocToken, LocString), KConfigError> {
        let cmd = self.next().unwrap();

        let Some(str_lit) = self.next() else {
            return Err(KConfigError::missing(Expected::StringLiteral, cmd.location()));
        };

        let Some(str_lit) = str_lit.string_literal_value() else {
            return Err(KConfigError::unexpected(str_lit, Expected::StringLiteral, str_lit.location()));
        };

        if require_eol {
            if let Some(unexpected) = self.next() {
                return Err(KConfigError::unexpected(unexpected, Expected::Eol, unexpected.location()));
            }
        }

        let str_lit = str_lit.to_loc_string();

        Ok((cmd, str_lit))
    }

    /// Read an `if <expr>` expression, if present.
    pub fn read_if_expr(&mut self, require_eof: bool) -> Result<Option<LocExpr>, KConfigError> {
        let Some(if_token) = self.next() else {
            return Ok(None);
        };

        if if_token.token != Token::If {
            return Err(KConfigError::unexpected(if_token, Expected::IfOrEol, if_token.location()));
        }

        let expr = LocExpr::parse(if_token.location(), self)?;

        if require_eof {
            if let Some(unexpected) = self.next() {
                return Err(KConfigError::unexpected(unexpected, Expected::Eol, unexpected.location()));
            }
        }

        Ok(Some(expr))
    }

    /// Read the help text from a `help` block.
    ///
    /// This is tokenized as [`Token::Help`] followed by a [`Token::StrLit`].
    ///
    /// If the line is not a `help` block, this returns an error.
    pub fn read_help(&mut self) -> Result<LocString, KConfigError> {
        let cmd = self.next().unwrap();

        if cmd.token != Token::Help {
            return Err(KConfigError::unexpected(cmd, Expected::Help, cmd.location()));
        }

        let Some(text) = self.next() else {
            return Err(KConfigError::missing(Expected::StringLiteral, cmd.location()));
        };

        let Some(text) = text.string_literal_value() else {
            return Err(KConfigError::unexpected(text, Expected::StringLiteral, text.location()));
        };

        if let Some(unexpected) = self.peek() {
            return Err(KConfigError::unexpected(unexpected, Expected::Eol, unexpected.location()));
        };

        let text = text.to_loc_string();
        Ok(text)
    }
}

impl<'buf> Iterator for TokenLine<'buf> {
    type Item = &'buf LocToken;

    fn next(&mut self) -> Option<Self::Item> {
        match self.peek() {
            Some(c) => {
                self.offset += 1;
                Some(c)
            }
            None => None,
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let n = self.base.len() - self.offset;
        (n, Some(n))
    }
}

impl<'buf> FusedIterator for TokenLine<'buf> {}

/// Parse the input stream into lines of tokens.
pub fn parse_stream(mut chars: PeekableChars) -> Result<Vec<Vec<LocToken>>, KConfigError> {
    let mut lines = vec![];

    loop {
        let line = parse_line(&mut chars)?;
        if line.is_empty() {
            break;
        }

        lines.push(line);
    }

    Ok(lines)
}

/// Parse the next non-empty line from the stream.
///
/// This returns an empty vector if EOF is reached without parsing any tokens.
pub fn parse_line(chars: &mut PeekableChars) -> Result<Vec<LocToken>, KConfigError> {
    'outer: loop {
        let mut tokens = vec![];

        loop {
            let Some(c) = chars.peek() else {
                // EOF reached. Return what we have.
                return Ok(tokens);
            };

            match c {
                '#' | '\n' => {
                    if c == '#' {
                        parse_comment(chars)?;
                    } else {
                        _ = chars.next();
                    }

                    if tokens.is_empty() {
                        // This line is empty; continue parsing from the next line.
                        continue 'outer;
                    } else if tokens.len() == 1 && tokens[0].token == Token::Help {
                        // This is a help block. Parse the help text and return it as a string literal.
                        let start = chars.location();
                        tokens.push(LocToken::new(Token::StrLit(read_help_block(chars)?), start));
                        return Ok(tokens);
                    } else {
                        // This line is not empty; return what we have.
                        return Ok(tokens);
                    }
                }

                '"' | '\'' => {
                    let start = chars.location();
                    let s = parse_string_literal(chars, c)?;
                    tokens.push(LocToken::new(Token::StrLit(s), start));
                }

                '+' | '-' | '0'..='9' => {
                    let start = chars.location();
                    let value = parse_integer_literal(chars)?;
                    tokens.push(LocToken::new(Token::IntLit(value), start));
                }

                c if c.is_whitespace() => {
                    _ = chars.next();
                }

                c if c.is_alphabetic() || c == '_' => {
                    let token = parse_keyword_or_symbol(chars)?;
                    tokens.push(token);
                }

                '&' if chars.starts_with("&&") => {
                    let start = chars.location();
                    _ = chars.next();
                    _ = chars.next();
                    tokens.push(LocToken::new(Token::And, start));
                }

                '|' if chars.starts_with("||") => {
                    let start = chars.location();
                    _ = chars.next();
                    _ = chars.next();
                    tokens.push(LocToken::new(Token::Or, start));
                }

                '=' => {
                    let start = chars.location();
                    _ = chars.next();
                    tokens.push(LocToken::new(Token::Eq, start));
                }

                '!' => {
                    let start = chars.location();
                    _ = chars.next();
                    let op = if chars.peek() == Some('=') {
                        _ = chars.next();
                        Token::Ne
                    } else {
                        Token::Not
                    };

                    tokens.push(LocToken::new(op, start));
                }

                '(' => {
                    let start = chars.location();
                    _ = chars.next();
                    tokens.push(LocToken::new(Token::LParen, start));
                }

                ')' => {
                    let start = chars.location();
                    _ = chars.next();
                    tokens.push(LocToken::new(Token::RParen, start));
                }

                '<' => {
                    let start = chars.location();
                    _ = chars.next();
                    let op = if chars.peek() == Some('=') {
                        _ = chars.next();
                        Token::Le
                    } else {
                        Token::Lt
                    };

                    tokens.push(LocToken::new(op, start));
                }

                '>' => {
                    let start = chars.location();
                    _ = chars.next();
                    let op = if chars.peek() == Some('=') {
                        _ = chars.next();
                        Token::Ge
                    } else {
                        Token::Gt
                    };

                    tokens.push(LocToken::new(op, start));
                }

                '\\' if chars.starts_with("\\\n") => {
                    // Line continuation. Skip the backslash and newline.
                    _ = chars.next();
                    _ = chars.next();
                }

                _ => return Err(KConfigError::syntax(c, chars.location())),
            }
        }
    }
}

/// Read a help block from the stream.
///
/// The first line of the help block determines the indentation level of the rest of the block.
/// The block continues until a non-empty line is found that is indented less than the first line.
///
/// The help text is returned as a string literal.
fn read_help_block(chars: &mut PeekableChars) -> Result<String, KConfigError> {
    let mut help = String::new();

    // Get the indentation level of the first line.
    let indent = parse_hws0(chars)?;

    if indent.is_empty() {
        let start = chars.location();
        let c = chars.peek().map(|c| c.to_string()).unwrap_or_else(|| "<EOF>".to_string());
        return Err(KConfigError::unexpected(c, Expected::Whitespace, start));
    }

    help.push_str(chars.read_until('\n'));

    while !chars.is_empty() {
        if chars.starts_with(indent) {
            // This line is indented with the first line. Add it to the help text.
            chars.advance(indent.len());
            help.push_str(chars.read_until('\n'));
        } else if chars.starts_with('\n') {
            // Empty line. Add it to the help text.
            _ = chars.next();
            help.push('\n');
        } else {
            // This line is not indented with the first line. Stop parsing help text.
            break;
        }
    }

    Ok(help)
}
