use crate::parser::{Expected, KConfigError, PeekableChars};

pub fn parse_hws0<'a, 'b>(chars: &'b mut PeekableChars<'a>) -> Result<&'a str, KConfigError>
where
    'a: 'b,
{
    // Remember where we started.
    let start = chars.offset();

    loop {
        match chars.peek() {
            Some('\\') => {
                let Some(c) = chars.peek_at(1) else {
                    return Err(KConfigError::unexpected_eof(Expected::Any, chars.location()));
                };

                if c.is_whitespace() {
                    _ = chars.next();
                    _ = chars.next();
                } else {
                    break;
                }
            }
            Some(c) if c.is_whitespace() => {
                _ = chars.next();
            }
            _ => break,
        }
    }

    // Return the slice of the original string that we consumed.
    let end = chars.offset();
    Ok(&chars.base_str()[start..end])
}
