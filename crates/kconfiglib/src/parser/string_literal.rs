//! String literal parsing. This is based on the example from
//! [nom](https://github.com/rust-bakery/nom/blob/main/examples/string.rs).
//!
//! A string is enclosed by double quotes (`"`) and can contain zero or more fragments consisting of:
//! * Any raw unescaped codepoint except `\\`` and `"`.
//! * One of the following escape sequences: `\\a`, `\\b`, `\\f`, `\\n`, `\\r`, `\\t`, `\\v`, `\\"`, `\\\\`
//! * A whitespace escape sequence of the form `\\[ \t\v\f]`.
//! * An octal escape sequence of the form `\\[0-7]{3}`.
//! * A hex escape sequence of the form `\\x[0-9a-fA-F]{2}`.
//! * A unicode escape sequence of the form `\\u{[0-9a-fA-F]{1,6}}`.

use crate::parser::{Expected, KConfigError, PeekableChars};

/// Read a string literal.
pub fn parse_string_literal(chars: &mut PeekableChars, end_token: char) -> Result<String, KConfigError> {
    let start = chars.location().clone();

    let Some(c) = chars.next() else {
        return Err(KConfigError::unexpected_eof(end_token, &start));
    };

    if c != end_token {
        return Err(KConfigError::unexpected(c, end_token, &start));
    }

    let mut interior = String::new();

    loop {
        let Some(c) = chars.next() else {
            return Err(KConfigError::unexpected_eof(end_token, &start));
        };

        if c == end_token {
            break;
        } else if c == '\\' {
            parse_escape(chars, &mut interior)?;
        } else {
            interior.push(c);
        }
    }

    Ok(interior)
}

/// Parse a string escape sequence.
pub(crate) fn parse_escape(chars: &mut PeekableChars, interior: &mut String) -> Result<(), KConfigError> {
    let start = chars.location().clone();

    let Some(c) = chars.next() else {
        return Err(KConfigError::unexpected_eof(Expected::Any, &start));
    };

    match c {
        'a' => interior.push('\u{07}'), // alarm (BEL)
        'b' => interior.push('\u{08}'), // backspace (BS)
        'e' => interior.push('\u{1B}'), // escape (ESC)
        'f' => interior.push('\u{0C}'), // form feed (FF)
        'n' => interior.push('\n'),     // newline (LF)
        'r' => interior.push('\r'),     // carriage return (CR)
        't' => interior.push('\t'),     // horizontal tab (TAB)
        'v' => interior.push('\u{0B}'), // vertical tab (VT)
        '\\' => interior.push('\\'),    // backslash
        '\'' => interior.push('\''),    // single quote
        '/' => interior.push('/'),      // forward slash
        '"' => interior.push('"'),      // double quote
        'x' => interior.push(parse_hex_escape(chars)?),
        'u' => interior.push(parse_unicode_escape(chars)?),
        c if c.is_whitespace() => {
            // Consume all whitespace
            _ = chars.next();
            loop {
                let Some(c) = chars.peek() else {
                    break;
                };

                if !c.is_whitespace() {
                    break;
                }

                _ = chars.next();
            }
        }
        c => return Err(KConfigError::unexpected(c, "abefnrtv\\/'\"xu", &start)),
    }
    Ok(())
}

/// Parse a hex escape sequence, continuing until a non-hex character is found.
fn parse_hex_escape(chars: &mut PeekableChars) -> Result<char, KConfigError> {
    let start = chars.location().clone();
    let mut hex = String::new();

    let Some(c) = chars.next() else {
        return Err(KConfigError::unexpected_eof(Expected::HexDigit, &start));
    };

    if !c.is_ascii_hexdigit() {
        return Err(KConfigError::unexpected(c, Expected::HexDigit, &start));
    }

    loop {
        let Some(c) = chars.peek() else {
            return Err(KConfigError::unexpected_eof(Expected::Any, &start));
        };

        if !c.is_ascii_hexdigit() {
            break;
        }

        _ = chars.next();
        hex.push(c);
    }

    let value = u32::from_str_radix(&hex, 16).unwrap();
    let Some(c) = char::from_u32(value) else {
        return Err(KConfigError::invalid_unicode(value, &start));
    };

    Ok(c)
}

/// Parse a unicode escape sequence.
fn parse_unicode_escape(chars: &mut PeekableChars) -> Result<char, KConfigError> {
    let start = chars.location().clone();
    let Some(c) = chars.next() else {
        return Err(KConfigError::unexpected_eof(Expected::UnicodeEscape, &start));
    };

    let mut hex = String::new();

    if c == '{' {
        loop {
            let Some(c) = chars.next() else {
                return Err(KConfigError::unexpected_eof(Expected::UnicodeEscape, chars.location()));
            };

            if c == '}' {
                break;
            }

            if !c.is_ascii_hexdigit() {
                return Err(KConfigError::unexpected(c, Expected::HexDigit, chars.location()));
            }

            hex.push(c);
        }

        if hex.is_empty() {
            return Err(KConfigError::unexpected('}', Expected::HexDigit, chars.location()));
        }
    } else if c.is_ascii_hexdigit() {
        // Get three more hex digits
        hex.push(c);

        for _ in 0..3 {
            let current = chars.location().clone();

            let Some(c) = chars.next() else {
                return Err(KConfigError::unexpected_eof(Expected::HexDigit, &current));
            };

            if !c.is_ascii_hexdigit() {
                return Err(KConfigError::unexpected(c, Expected::HexDigit, &current));
            }

            hex.push(c);
        }
    } else {
        return Err(KConfigError::unexpected(c, Expected::UnicodeEscape, &start));
    }

    let value = u32::from_str_radix(&hex, 16).unwrap();
    let Some(c) = char::from_u32(value) else {
        return Err(KConfigError::invalid_unicode(value, &start));
    };

    Ok(c)
}
