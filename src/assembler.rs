use std::collections::HashMap;

use crate::cpu::{Cpu, MEM_SIZE};

#[derive(Clone, Default)]
pub struct AsmRow {
    pub address: Option<u16>,
    pub words: [Option<u16>; 3],
    pub error: [bool; 4],
}

#[derive(Clone)]
pub struct AsmResult {
    pub code: Vec<u16>,
    pub line_map: HashMap<u16, usize>,
    pub errors: Vec<String>,
    pub rows: Vec<AsmRow>,
}

pub fn instr_info(mnemonic: &str) -> Option<(usize, u16)> {
    match mnemonic {
        "HLT" => Some((0, 0x001)),
        "NOP" => Some((0, 0x002)),
        "PUSH" => Some((0, 0x005)),
        "POP" => Some((0, 0x006)),
        "RET" => Some((0, 0x007)),
        "LD" | "ST" | "ADD" | "SUB" | "AND" | "OR" | "XOR" |
        "BRA" | "BRZ" | "BLT" | "CALL" | "IN" | "OUT" => Some((1, 0x000)),
        "CMP" | "OUTI" => Some((2, 0x000)),
        _ => None,
    }
}

fn instruction_operand_count(mnemonic: &str) -> Option<usize> {
    instr_info(mnemonic).map(|(n, _)| n)
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum OperandKind {
    Memory,
    Immediate,
}

fn parse_hex_or_label(s: &str, labels: &HashMap<String, u16>) -> Result<u16, String> {
    let s = s.trim();
    if s.is_empty() {
        return Err("空值".to_string());
    }
    if let Some(hex) = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
        if hex.len() > 4 {
            return Err(format!("十六进制数超过 4 位: {}", s));
        }
        let padded = format!("{:0>4}", hex);
        u16::from_str_radix(&padded, 16).map_err(|e| e.to_string())
    } else {
        let lower = s.to_lowercase();
        if !is_valid_label(&lower) {
            return Err(format!("非法标号: {}", s));
        }
        labels
            .get(&lower)
            .copied()
            .ok_or_else(|| format!("未定义标号: {}", s))
    }
}

fn is_valid_label(s: &str) -> bool {
    if s.is_empty() {
        return false;
    }
    let mut chars = s.chars();
    let first = chars.next().unwrap();
    if !first.is_ascii_lowercase() && first != '_' {
        return false;
    }
    for ch in chars {
        if !ch.is_ascii_lowercase() && !ch.is_ascii_digit() && ch != '_' {
            return false;
        }
    }
    true
}

fn parse_operand(s: &str, kind: OperandKind, labels: &HashMap<String, u16>) -> Result<u16, String> {
    let s = s.trim();
    match kind {
        OperandKind::Memory => {
            if !s.starts_with('[') || !s.ends_with(']') {
                return Err(format!("内存操作数必须用 [] 包围: {}", s));
            }
            let inner = &s[1..s.len() - 1];
            parse_hex_or_label(inner, labels)
        }
        OperandKind::Immediate => {
            if s.contains('[') || s.contains(']') {
                return Err(format!("该指令不支持间接寻址: {}", s));
            }
            parse_hex_or_label(s, labels)
        }
    }
}

fn resolve_instruction(mnemonic: &str, operands: &[&str]) -> Result<(u16, Vec<OperandKind>), String> {
    let n = operands.len();
    match mnemonic {
        "HLT" => Ok((0x001, vec![])),
        "NOP" => Ok((0x002, vec![])),
        "PUSH" => Ok((0x005, vec![])),
        "POP" => Ok((0x006, vec![])),
        "RET" => Ok((0x007, vec![])),
        "LD" => {
            if n != 1 { return Err("LD 需要 1 个操作数".to_string()); }
            let op = operands[0].trim();
            if op.starts_with('[') && op.ends_with(']') {
                Ok((0x001, vec![OperandKind::Memory]))
            } else if op.contains('[') || op.contains(']') {
                Err(format!("LD 操作数格式错误: {}", op))
            } else {
                Ok((0x003, vec![OperandKind::Immediate]))
            }
        },
        "ST" => {
            if n != 1 { return Err("ST 需要 1 个操作数".to_string()); }
            let op = operands[0].trim();
            if op.starts_with('[') && op.ends_with(']') {
                Ok((0x002, vec![OperandKind::Memory]))
            } else {
                Err(format!("ST 必须使用 [addr] 指定内存地址: {}", op))
            }
        },
        "ADD" => {
            if n != 1 { return Err("ADD 需要 1 个操作数".to_string()); }
            let op = operands[0].trim();
            if op.starts_with('[') && op.ends_with(']') {
                Ok((0x004, vec![OperandKind::Memory]))
            } else {
                Ok((0x021, vec![OperandKind::Immediate]))
            }
        },
        "SUB" => {
            if n != 1 { return Err("SUB 需要 1 个操作数".to_string()); }
            let op = operands[0].trim();
            if op.starts_with('[') && op.ends_with(']') {
                Ok((0x005, vec![OperandKind::Memory]))
            } else {
                Ok((0x022, vec![OperandKind::Immediate]))
            }
        },
        "AND" => {
            if n != 1 { return Err("AND 需要 1 个操作数".to_string()); }
            let op = operands[0].trim();
            if op.starts_with('[') && op.ends_with(']') {
                Ok((0x006, vec![OperandKind::Memory]))
            } else {
                Ok((0x023, vec![OperandKind::Immediate]))
            }
        },
        "OR" => {
            if n != 1 { return Err("OR 需要 1 个操作数".to_string()); }
            let op = operands[0].trim();
            if op.starts_with('[') && op.ends_with(']') {
                Ok((0x007, vec![OperandKind::Memory]))
            } else {
                Ok((0x024, vec![OperandKind::Immediate]))
            }
        },
        "XOR" => {
            if n != 1 { return Err("XOR 需要 1 个操作数".to_string()); }
            let op = operands[0].trim();
            if op.starts_with('[') && op.ends_with(']') {
                Ok((0x008, vec![OperandKind::Memory]))
            } else {
                Ok((0x025, vec![OperandKind::Immediate]))
            }
        },
        "BRA" => Ok((0x009, vec![OperandKind::Immediate])),
        "BRZ" => Ok((0x00A, vec![OperandKind::Immediate])),
        "BLT" => Ok((0x00B, vec![OperandKind::Immediate])),
        "CALL" => Ok((0x011, vec![OperandKind::Immediate])),
        "IN" => Ok((0x012, vec![OperandKind::Immediate])),
        "OUT" => Ok((0x013, vec![OperandKind::Immediate])),
        "CMP" => {
            if n != 2 { return Err("CMP 需要 2 个操作数".to_string()); }
            let op1 = operands[0].trim();
            let op2 = operands[1].trim();
            if !(op1.starts_with('[') && op1.ends_with(']')) {
                return Err(format!("CMP 第一个操作数必须是内存地址 [addr]: {}", op1));
            }
            if op2.starts_with('[') && op2.ends_with(']') {
                Ok((0x004, vec![OperandKind::Memory, OperandKind::Memory]))
            } else {
                Ok((0x006, vec![OperandKind::Memory, OperandKind::Immediate]))
            }
        },
        "OUTI" => Ok((0x005, vec![OperandKind::Immediate, OperandKind::Immediate])),
        _ => Err(format!("未知指令: {}", mnemonic)),
    }
}

pub fn assemble_table(rows: &[[String; 4]]) -> AsmResult {
    let mut labels = HashMap::new();
    let mut errors = Vec::new();
    let mut row_infos: Vec<AsmRow> = Vec::with_capacity(rows.len());

    let mut addr: u16 = 0;
    let mut address_known = true;
    for (row_idx, row) in rows.iter().enumerate() {
        let mut info = AsmRow::default();
        let symbol = row[0].trim();
        if !symbol.is_empty() {
            let lower = symbol.to_lowercase();
            if !is_valid_label(&lower) {
                errors.push(format!("行 {}: 非法标号 {}", row_idx + 1, symbol));
            } else if labels.contains_key(&lower) {
                errors.push(format!("行 {}: 重复标号 {}", row_idx + 1, symbol));
            } else {
                labels.insert(lower, addr);
            }
        }
        let col2 = row[1].trim();
        if col2.is_empty() {
            errors.push(format!("行 {}: 缺少指令或数据", row_idx + 1));
            info.address = None;
            info.error[0] = true;
            address_known = false;
            row_infos.push(info);
            continue;
        }
        let mnemonic = col2.to_uppercase();
        if let Some(n) = instruction_operand_count(&mnemonic) {
            info.address = if address_known { Some(addr) } else { None };
            if !address_known {
                info.error[0] = true;
            }
            let len = 1 + n as u16;
            if address_known {
                addr = addr.wrapping_add(len);
            }
        } else {
            let mut count = 0;
            for i in 1..4 {
                if !row[i].trim().is_empty() {
                    count += 1;
                }
            }
            info.address = if address_known { Some(addr) } else { None };
            if !address_known {
                info.error[0] = true;
            }
            if address_known {
                addr = addr.wrapping_add(count);
            }
        }
        row_infos.push(info);
    }

    let mut code = Vec::new();
    let mut line_map = HashMap::new();
    let mut addr: u16 = 0;
    let mut current_addr_known = true;
    for (row_idx, row) in rows.iter().enumerate() {
        let info = &mut row_infos[row_idx];
        if !current_addr_known {
            info.address = None;
            for e in &mut info.error {
                *e = true;
            }
            continue;
        }
        let col2 = row[1].trim();
        if col2.is_empty() {
            current_addr_known = false;
            info.address = None;
            for e in &mut info.error {
                *e = true;
            }
            continue;
        }
        let mnemonic = col2.to_uppercase();
        if let Some(n) = instruction_operand_count(&mnemonic) {
            info.address = Some(addr);
            let operands: Vec<&str> = (0..n).map(|i| row[2 + i].trim()).collect();
            match resolve_instruction(&mnemonic, &operands) {
                Ok((sub, kinds)) => {
                    let machine = ((n as u16) << 12) | sub;
                    info.words[0] = Some(machine);
                    code.push(machine);
                    line_map.insert(addr, row_idx);
                    let mut addr_cur = addr.wrapping_add(1);
                    for (i, kind) in kinds.iter().enumerate() {
                        let op_str = operands[i];
                        match parse_operand(op_str, *kind, &labels) {
                            Ok(val) => {
                                info.words[1 + i] = Some(val);
                                code.push(val);
                                line_map.insert(addr_cur, row_idx);
                                addr_cur = addr_cur.wrapping_add(1);
                            }
                            Err(e) => {
                                errors.push(format!("行 {}: 操作数错误: {}", row_idx + 1, e));
                                info.words[1 + i] = None;
                                info.error[2 + i] = true;
                                addr_cur = addr_cur.wrapping_add(1);
                            }
                        }
                    }
                    addr = addr_cur;
                }
                Err(e) => {
                    errors.push(format!("行 {}: {}", row_idx + 1, e));
                    info.error[1] = true;
                    addr = addr.wrapping_add(1 + n as u16);
                }
            }
        } else {
            info.address = Some(addr);
            let mut addr_cur = addr;
            let mut word_idx = 0;
            for i in 1..4 {
                let cell = row[i].trim();
                if cell.is_empty() {
                    continue;
                }
                match parse_hex_or_label(cell, &labels) {
                    Ok(val) => {
                        info.words[word_idx] = Some(val);
                        code.push(val);
                        line_map.insert(addr_cur, row_idx);
                        addr_cur = addr_cur.wrapping_add(1);
                        word_idx += 1;
                    }
                    Err(e) => {
                        errors.push(format!("行 {}: 数据错误: {}", row_idx + 1, e));
                        info.words[word_idx] = None;
                        info.error[1 + word_idx] = true;
                        addr_cur = addr_cur.wrapping_add(1);
                        word_idx += 1;
                    }
                }
            }
            addr = addr_cur;
        }
    }

    AsmResult {
        code,
        line_map,
        errors,
        rows: row_infos,
    }
}

pub fn load_into_cpu(cpu: &mut Cpu, asm: &AsmResult) {
    cpu.reset();
    for (i, word) in asm.code.iter().enumerate() {
        if i < MEM_SIZE {
            cpu.mem[i] = *word;
        }
    }
    cpu.pc = 0;
    cpu.halted = false;
}
