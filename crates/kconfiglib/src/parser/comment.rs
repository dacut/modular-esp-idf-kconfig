use crate::parser::{string_literal::parse_escape, Expected, KConfigError, PeekableChars};

/// Parse a comment from the stream.
///
/// The stream must be pointing at a '#' character. This and the rest of the line, up to and including the newline,
/// will be consumed.
pub fn parse_comment(chars: &mut PeekableChars) -> Result<(), KConfigError> {
    let Some(c) = chars.next() else {
        return Err(KConfigError::unexpected_eof(Expected::Any, chars.location()));
    };

    if c != '#' {
        return Err(KConfigError::unexpected(c, "#", chars.location()));
    }

    // Eat the # character; don't include it in the comment.
    let mut comment = String::new();

    loop {
        let Some(c) = chars.next() else {
            break;
        };
        if c == '\n' {
            break;
        } else if c == '\\' {
            parse_escape(chars, &mut comment)?;
        } else {
            comment.push(c);
        }
    }

    Ok(())
}
