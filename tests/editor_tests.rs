use lmc::editor::{cursor_line_col, line_end_byte, line_start_byte, set_cursor_from_line_col};

#[test]
fn cursor_line_col_counts_lines_and_columns() {
    let text = "ab\ncde\n";
    assert_eq!(cursor_line_col(text, 0), (0, 0));
    assert_eq!(cursor_line_col(text, 2), (0, 2));
    assert_eq!(cursor_line_col(text, 3), (1, 0));
    assert_eq!(cursor_line_col(text, 5), (1, 2));
}

#[test]
fn line_helpers_return_byte_offsets() {
    let text = "ab\ncde\nfg";
    assert_eq!(line_start_byte(text, 0), 0);
    assert_eq!(line_start_byte(text, 1), 3);
    assert_eq!(line_start_byte(text, 2), 7);
    assert_eq!(line_end_byte(text, 0), 2);
    assert_eq!(line_end_byte(text, 1), 6);
    assert_eq!(line_end_byte(text, 2), 9);
}

#[test]
fn set_cursor_from_line_col_clamps_to_line_end() {
    let text = "ab\ncde\nfg";
    let mut cursor = 0;
    set_cursor_from_line_col(text, &mut cursor, 1, 10);
    assert_eq!(cursor, 6);
    set_cursor_from_line_col(text, &mut cursor, 2, 1);
    assert_eq!(cursor, 8);
}
