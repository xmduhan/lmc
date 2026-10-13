pub fn cursor_line_col(text: &str, cursor: usize) -> (usize, usize) {
    let mut line = 0;
    let mut col = 0;
    for (i, ch) in text.char_indices() {
        if i >= cursor { break; }
        if ch == '\n' {
            line += 1;
            col = 0;
        } else {
            col += 1;
        }
    }
    (line, col)
}

pub fn line_start_byte(text: &str, line: usize) -> usize {
    if line == 0 { return 0; }
    let mut count = 0;
    for (i, ch) in text.char_indices() {
        if ch == '\n' {
            count += 1;
            if count == line {
                return i + 1;
            }
        }
    }
    text.len()
}

pub fn line_end_byte(text: &str, line: usize) -> usize {
    let start = line_start_byte(text, line);
    let rest = &text[start..];
    if let Some(pos) = rest.find('\n') {
        start + pos
    } else {
        text.len()
    }
}

pub fn set_cursor_from_line_col(text: &str, cursor: &mut usize, line: usize, col: usize) {
    let start = line_start_byte(text, line);
    let end = line_end_byte(text, line);
    let mut byte = start;
    let mut c = 0;
    for (i, ch) in text[start..end].char_indices() {
        if c >= col { break; }
        byte = start + i + ch.len_utf8();
        c += 1;
    }
    *cursor = byte.min(text.len());
}
