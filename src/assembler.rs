use std::collections::HashMap;

use crate::cpu::{Cpu, MEM_SIZE};

#[derive(Clone)]
pub struct AsmResult {
    pub code: Vec<u16>,
    pub line_map: HashMap<u16, usize>,
    pub errors: Vec<String>,
}

impl AsmResult {
    pub fn empty() -> Self {
        AsmResult {
            code: Vec::new(),
            line_map: HashMap::new(),
            errors: Vec::new(),
        }
    }
}

fn instr_info(mnemonic: &str) -> Option<(usize, u16)> {
    match mnemonic {
        "HLT" => Some((0, 0x001)),
        "NOP" => Some((0, 0x002)),
        "PUSH" => Some((0, 0x005)),
        "POP" => Some((0, 0x006)),
        "RET" => Some((0, 0x007)),
        "LDA" => Some((1, 0x001)),
        "STA" => Some((1, 0x002)),
        "LDI" => Some((1, 0x003)),
        "ADD" => Some((1, 0x004)),
        "SUB" => Some((1, 0x005)),
        "AND" => Some((1, 0x006)),
        "OR" => Some((1, 0x007)),
        "XOR" => Some((1, 0x008)),
        "BRA" => Some((1, 0x009)),
        "BRZ" => Some((1, 0x00A)),
        "BLT" => Some((1, 0x00B)),
        "CALL" => Some((1, 0x011)),
        "IN" => Some((1, 0x012)),
        "OUT" => Some((1, 0x013)),
        "CMP" => Some((2, 0x004)),
        "OUTI" => Some((2, 0x005)),
        _ => None,
    }
}

fn parse_operand(
    s: &str,
    labels: &HashMap<String, u16>,
    consts: &HashMap<String, u16>,
) -> Result<u16, String> {
    let s = s.trim();
    if s.is_empty() {
        return Err("缺少操作数".to_string());
    }
    if let Some(hex) = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
        u16::from_str_radix(hex, 16).map_err(|e| e.to_string())
    } else if let Ok(v) = s.parse::<u16>() {
        Ok(v)
    } else if let Some(v) = consts.get(s).or_else(|| labels.get(s)) {
        Ok(*v)
    } else {
        Err(format!("未知符号 {}", s))
    }
}

fn assemble(source: &str) -> AsmResult {
    let mut labels = HashMap::new();
    let mut consts = HashMap::new();
    let mut errors = Vec::new();
    let lines: Vec<&str> = source.split('\n').collect();
    let mut addr: u16 = 0;

    for (line_idx, raw) in lines.iter().enumerate() {
        let mut line = raw.split(';').next().unwrap_or("").trim();
        if line.is_empty() { continue; }
        let mut label_opt: Option<String> = None;
        if let Some(colon) = line.find(':') {
            let label = line[..colon].trim();
            if !label.is_empty() {
                label_opt = Some(label.to_string());
            }
            line = line[colon + 1..].trim();
            if line.is_empty() {
                if let Some(lbl) = label_opt {
                    if labels.contains_key(&lbl) || consts.contains_key(&lbl) {
                        errors.push(format!("行 {}: 重复符号 {}", line_idx + 1, lbl));
                    } else {
                        labels.insert(lbl, addr);
                    }
                }
                continue;
            }
        }
        let tokens: Vec<&str> = line.split_whitespace().collect();
        if tokens.is_empty() {
            if let Some(lbl) = label_opt {
                if labels.contains_key(&lbl) || consts.contains_key(&lbl) {
                    errors.push(format!("行 {}: 重复符号 {}", line_idx + 1, lbl));
                } else {
                    labels.insert(lbl, addr);
                }
            }
            continue;
        }
        if tokens[0].to_uppercase() == "EQU" {
            let name = label_opt.clone().unwrap_or_default();
            if name.is_empty() {
                errors.push(format!("行 {}: EQU 缺少名称", line_idx + 1));
                continue;
            }
            let value_tok = tokens.get(1).copied().unwrap_or("");
            match parse_operand(value_tok, &labels, &consts) {
                Ok(v) => { consts.insert(name, v); }
                Err(e) => errors.push(format!("行 {}: EQU 值错误 {}", line_idx + 1, e)),
            }
            continue;
        }
        if tokens.len() >= 2 && tokens[1].to_uppercase() == "EQU" {
            let name = tokens[0].to_string();
            let value_tok = tokens.get(2).copied().unwrap_or("");
            match parse_operand(value_tok, &labels, &consts) {
                Ok(v) => { consts.insert(name, v); }
                Err(e) => errors.push(format!("行 {}: EQU 值错误 {}", line_idx + 1, e)),
            }
            continue;
        }
        if let Some(lbl) = label_opt {
            if labels.contains_key(&lbl) || consts.contains_key(&lbl) {
                errors.push(format!("行 {}: 重复符号 {}", line_idx + 1, lbl));
            } else {
                labels.insert(lbl, addr);
            }
        }
        let mnemonic = tokens[0].to_uppercase();
        if let Some((n, _)) = instr_info(&mnemonic) {
            addr = addr.wrapping_add(1 + n as u16);
        } else {
            errors.push(format!("行 {}: 未知指令 {}", line_idx + 1, mnemonic));
        }
    }

    let mut code = Vec::new();
    let mut line_map = HashMap::new();
    let mut addr: u16 = 0;
    for (line_idx, raw) in lines.iter().enumerate() {
        let mut line = raw.split(';').next().unwrap_or("").trim();
        if line.is_empty() { continue; }
        if let Some(colon) = line.find(':') {
            line = line[colon + 1..].trim();
            if line.is_empty() { continue; }
        }
        let tokens: Vec<&str> = line.split_whitespace().collect();
        if tokens.is_empty() { continue; }
        if tokens[0].to_uppercase() == "EQU" { continue; }
        if tokens.len() >= 2 && tokens[1].to_uppercase() == "EQU" { continue; }
        let mnemonic = tokens[0].to_uppercase();
        if let Some((n, sub)) = instr_info(&mnemonic) {
            let machine = ((n as u16) << 12) | sub;
            code.push(machine);
            line_map.insert(addr, line_idx);
            addr = addr.wrapping_add(1);
            for i in 0..n {
                let tok = tokens.get(1 + i).copied().unwrap_or("");
                let val = match parse_operand(tok, &labels, &consts) {
                    Ok(v) => v,
                    Err(e) => {
                        errors.push(format!("行 {}: 操作数错误 {} ({})", line_idx + 1, tok, e));
                        0
                    }
                };
                code.push(val);
                line_map.insert(addr, line_idx);
                addr = addr.wrapping_add(1);
            }
            if tokens.len() > 1 + n {
                errors.push(format!("行 {}: 指令 {} 操作数过多", line_idx + 1, mnemonic));
            }
        }
    }

    AsmResult { code, line_map, errors }
}

pub fn assemble_and_load(
    cpu: &mut Cpu,
    source: &str,
    asm: &mut AsmResult,
    status: &mut String,
    dirty: &mut bool,
) {
    let result = assemble(source);
    *asm = result;
    if asm.errors.is_empty() {
        cpu.reset();
        for (i, word) in asm.code.iter().enumerate() {
            if i < MEM_SIZE {
                cpu.mem[i] = *word;
            }
        }
        cpu.pc = 0;
        cpu.halted = false;
        *dirty = false;
        *status = format!("汇编成功: {} 字", asm.code.len());
    } else {
        *status = format!("汇编错误: {}", asm.errors[0]);
    }
}
