use lmc::assembler::assemble_table;

fn make_table(rows: &[[&str; 4]]) -> Vec<[String; 4]> {
    rows.iter()
        .map(|r| [
            r[0].to_string(),
            r[1].to_string(),
            r[2].to_string(),
            r[3].to_string(),
        ])
        .collect()
}

#[test]
fn assembles_ldi_and_out() {
    let table = make_table(&[
        ["", "LDI", "0x002A", ""],
        ["", "OUT", "0xEA60", ""],
    ]);
    let asm = assemble_table(&table);
    assert!(asm.errors.is_empty(), "{:?}", asm.errors);
    assert_eq!(asm.code, vec![0x1003, 42, 0x1013, 60000]);
}

#[test]
fn assembles_labels_and_data() {
    let table = make_table(&[
        ["", "LDA", "[one]", ""],
        ["", "OUT", "0xEA60", ""],
        ["one", "0x0007", "", ""],
    ]);
    let asm = assemble_table(&table);
    assert!(asm.errors.is_empty(), "{:?}", asm.errors);
    assert_eq!(asm.code, vec![0x1001, 4, 0x1013, 60000, 7]);
}

#[test]
fn reports_unknown_mnemonic() {
    let table = make_table(&[["", "FOO", "", ""]]);
    let asm = assemble_table(&table);
    assert!(!asm.errors.is_empty());
}

#[test]
fn memory_operand_requires_brackets() {
    let table = make_table(&[["", "LDA", "0x0000", ""]]);
    let asm = assemble_table(&table);
    assert!(!asm.errors.is_empty());
}
