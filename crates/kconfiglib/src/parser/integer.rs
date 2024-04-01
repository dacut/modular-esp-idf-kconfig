use crate::parser::{Expected, KConfigError, Located, PeekableChars, Token};

pub fn parse_int_hex_literal(chars: &mut PeekableChars) -> Result<Token, KConfigError> {
    let start = chars.location();

    let Some(c) = chars.peek() else {
        return Err(KConfigError::unexpected_eof(Expected::Any, start));
    };

    if c == '+' || c == '-' {
        parse_dec_literal(chars)
    } else if chars.starts_with("0x") || chars.starts_with("0X") {
        parse_hex_literal(chars)
    } else if chars.starts_with('0') {
        parse_dec_oct_literal(chars)
    } else if !c.is_ascii_digit() {
        Err(KConfigError::unexpected(c, Expected::IntegerLiteral, start))
    } else {
        parse_dec_literal(chars)
    }
}

fn parse_dec_literal(chars: &mut PeekableChars) -> Result<Token, KConfigError> {
    let mut literal = String::new();
    let start = chars.location();

    let Some(c) = chars.peek() else {
        return Err(KConfigError::unexpected_eof(Expected::IntegerLiteral, start));
    };

    if c == '+' || c == '-' {
        literal.push(c);
        _ = chars.next();
    }

    loop {
        let Some(c) = chars.peek() else {
            break;
        };

        if c.is_ascii_digit() {
            literal.push(c);
            _ = chars.next();
        } else {
            break;
        }
    }

    #[allow(clippy::from_str_radix_10)]
    let value = i64::from_str_radix(&literal, 10).map_err(|_| KConfigError::invalid_integer(literal, start))?;

    Ok(Token::IntLit(value))
}

fn parse_hex_literal(chars: &mut PeekableChars) -> Result<Token, KConfigError> {
    let mut literal = String::new();
    let start = chars.location();

    let Some(c) = chars.next() else {
        return Err(KConfigError::unexpected_eof(Expected::IntegerLiteral, start));
    };
    if c != '0' {
        return Err(KConfigError::unexpected(c, Expected::IntegerLiteral, start));
    }

    let Some(radix_char) = chars.next() else {
        return Err(KConfigError::unexpected_eof(Expected::IntegerLiteral, start));
    };
    if radix_char != 'x' && radix_char != 'X' {
        return Err(KConfigError::unexpected(c, Expected::IntegerLiteral, start));
    }

    loop {
        let Some(c) = chars.peek() else {
            break;
        };

        if c.is_ascii_hexdigit() {
            literal.push(c);
            _ = chars.next();
        } else {
            break;
        }
    }

    if literal.is_empty() {
        return Err(KConfigError::invalid_integer(format!("0{radix_char}"), start));
    }

    let value = u64::from_str_radix(&literal, 16)
        .map_err(|_| KConfigError::invalid_integer(format!("0{radix_char}{literal}"), start))?;

    Ok(Token::HexLit(value))
}

fn parse_dec_oct_literal(chars: &mut PeekableChars) -> Result<Token, KConfigError> {
    let mut literal = String::new();
    let start = chars.location();

    let Some(c) = chars.peek() else {
        return Err(KConfigError::unexpected_eof(Expected::IntegerLiteral, start));
    };
    if c != '0' {
        return Err(KConfigError::unexpected(c, Expected::IntegerLiteral, start));
    }

    loop {
        let Some(c) = chars.peek() else {
            break;
        };

        if ('0'..='7').contains(&c) {
            literal.push(c);
            _ = chars.next();
        } else {
            break;
        }
    }

    if literal.is_empty() || literal == "0" {
        Ok(Token::IntLit(0))
    } else {
        let value = u64::from_str_radix(&literal, 8).map_err(|_| KConfigError::invalid_integer(format!("0{literal}"), start))?;
        Ok(Token::HexLit(value))
    }
}
