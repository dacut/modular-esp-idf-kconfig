use crate::parser::{Expected, KConfigError, Located, PeekableChars};

pub fn parse_integer_literal(chars: &mut PeekableChars) -> Result<i64, KConfigError> {
    let start = chars.location();

    let Some(c) = chars.peek() else {
        return Err(KConfigError::unexpected_eof(Expected::Any, start));
    };

    if c == '+' || c == '-' {
        parse_decimal_literal(chars)
    } else if chars.starts_with("0x") || chars.starts_with("0X") {
        parse_hex_literal(chars)
    } else if chars.starts_with('0') {
        parse_octal_literal(chars)
    } else if !c.is_ascii_digit() {
        Err(KConfigError::unexpected(c, Expected::IntegerLiteral, start))
    } else {
        parse_decimal_literal(chars)
    }
}

fn parse_decimal_literal(chars: &mut PeekableChars) -> Result<i64, KConfigError> {
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
    i64::from_str_radix(&literal, 10).map_err(|_| KConfigError::invalid_integer(literal, start))
}

fn parse_hex_literal(chars: &mut PeekableChars) -> Result<i64, KConfigError> {
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

    i64::from_str_radix(&literal, 16)
        .map_err(|_| KConfigError::invalid_integer(format!("0{radix_char}{literal}"), start))
}

fn parse_octal_literal(chars: &mut PeekableChars) -> Result<i64, KConfigError> {
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

    if literal.is_empty() {
        return Ok(0);
    }

    i64::from_str_radix(&literal, 16).map_err(|_| KConfigError::invalid_integer(format!("0{literal}"), start))
}
