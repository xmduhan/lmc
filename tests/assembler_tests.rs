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
fn assembles_ld_and_out() {
    let table = make_table(&[
        ["", "LD", "0x002A", ""],
        ["", "OUT", "0xEA60", ""],
    ]);
    let asm = assemble_table(&table);
    assert!(asm.errors.is_empty(), "{:?}", asm.errors);
    assert_eq!(asm.code, vec![0x1003, 42, 0x1013, 60000]);
}

#[test]
fn assembles_memory_ld_and_st() {
    let table = make_table(&[
        ["", "LD", "[0x0064]", ""],
        ["", "ST", "[0x0065]", ""],
    ]);
    let asm = assemble_table(&table);
    assert!(asm.errors.is_empty(), "{:?}", asm.errors);
    assert_eq!(asm.code, vec![0x1001, 100, 0x1002, 101]);
}

#[test]
fn assembles_immediate_arithmetic() {
    let table = make_table(&[
        ["", "ADD", "0x0001", ""],
        ["", "SUB", "0x0002", ""],
        ["", "AND", "0x0003", ""],
        ["", "OR", "0x0004", ""],
        ["", "XOR", "0x0005", ""],
    ]);
    let asm = assemble_table(&table);
    assert!(asm.errors.is_empty(), "{:?}", asm.errors);
    assert_eq!(asm.code, vec![0x1021, 1, 0x1022, 2, 0x1023, 3, 0x1024, 4, 0x1025, 5]);
}

#[test]
fn assembles_cmp_immediate() {
    let table = make_table(&[
        ["", "CMP", "[0x0064]", "0x0001"],
    ]);
    let asm = assemble_table(&table);
    assert!(asm.errors.is_empty(), "{:?}", asm.errors);
    assert_eq!(asm.code, vec![0x2006, 100, 1]);
}

#[test]
fn assembles_labels_and_data() {
    let table = make_table(&[
        ["", "LD", "[one]", ""],
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
fn st_requires_brackets() {
    let table = make_table(&[["", "ST", "0x0000", ""]]);
    let asm = assemble_table(&table);
    assert!(!asm.errors.is_empty());
}
