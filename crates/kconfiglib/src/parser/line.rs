use crate::parser::Location;

/// The result of reading a line, including the line itself, the remaining data, and the location of the next line.
#[derive(Debug)]
pub(crate) struct ReadLineResult<'a> {
    pub line: String,
    pub remaining: &'a str,
    pub next_location: Location,
}

/// Read a line from `data`. If a line ends with a backslash-newline, continue reading onto the next line, omitting
/// the backslash-newline combination in the output.
///
/// If the end of the file is reached without any content, return `None`.
pub(crate) fn read_line(mut location: Location, data: &str) -> Option<ReadLineResult<'_>> {
    let mut line = String::new();
    let mut last_was_backslash = false;
    let mut chars = data.chars();

    for c in &mut chars {
        if last_was_backslash {
            last_was_backslash = false;
            if c == '\n' {
                location.line += 1;
                location.column = 1;
                continue;
            }
        } else if c == '\\' {
            last_was_backslash = true;
            continue;
        } else if c == '\n' {
            location.line += 1;
            location.column = 1;
            return Some(ReadLineResult {
                line,
                remaining: chars.as_str(),
                next_location: location,
            });
        } else if c == '\t' {
            location.column = (location.column + 8) & !7;
        } else {
            location.column += 1;
            line.push(c);
        }
    }

    // Reached the end-of-file. If the last line was empty, return `None`.
    if line.is_empty() {
        None
    } else {
        Some(ReadLineResult {
            line,
            remaining: chars.as_str(),
            next_location: location,
        })
    }
}

/// Read the next non-empty line (i.e., a line consisting of more than just whitespace) from `data`.
/// If a line ends with a backslash-newline, continue reading onto the next line, omitting
/// the backslash-newline combination in the output.
///
/// If the end of the file is reached without any content, return `None`.
pub(crate) fn read_nonempty_line(mut location: Location, mut data: &str) -> Option<ReadLineResult> {
    loop {
        match read_line(location, data) {
            None => return None,
            Some(result) => {
                if result.line.trim().is_empty() {
                    location = result.next_location;
                    data = result.remaining;
                    continue;
                } else {
                    return Some(result);
                }
            }
        }
    }
}
