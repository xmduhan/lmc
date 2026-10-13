# 小人计算机扩展指令集架构规范

**版本 1.0**

---

## 目录

1. [概述](#一概述)
2. [寄存器](#二寄存器)
3. [内存布局](#三内存布局)
4. [指令格式](#四指令格式)
5. [指令表](#五指令表)
6. [标志位规则](#六标志位规则)
7. [栈与函数调用约定](#七栈与函数调用约定)
8. [内存映射 I/O](#八内存映射-io)
9. [指令速查](#九指令速查)
10. [代码示例](#十代码示例)
11. [设计取舍](#十一设计取舍)

---

## 一、概述

本架构是一种用于教学的小人计算机（Little Man Computer）扩展指令集。它在保留原始 LMC 直观性的基础上，增加了函数调用、条件分支、位运算和内存映射 I/O，使其能够支撑最基础的 C 语言编译。

**核心特征：**

- 字长 16 位
- 变长指令：0、1、2 个操作数
- 操作码高 4 位编码操作数个数
- 只有 4 个硬件寄存器：PC、ACC、Z、N
- 栈指针、帧指针、临时变量用内存模拟
- I/O 采用内存映射，端口位于固定地址区

---

## 二、寄存器

### 2.1 硬件寄存器

| 寄存器 | 宽度 | 作用 |
|--------|------|------|
| PC | 16 位 | 程序计数器，指向下一条要执行的指令地址 |
| ACC | 16 位 | 累加器，唯一运算寄存器 |

### 2.2 硬件标志位

| 标志 | 宽度 | 作用 |
|------|------|------|
| Z | 1 位 | 零标志，上一次运算结果为 0 时置 1 |
| N | 1 位 | 负标志，上一次运算结果为负时置 1 |

### 2.3 内存模拟寄存器

| 名称 | 内存地址 | 作用 |
|------|----------|------|
| SP | mem[0] | 栈指针，指向当前栈顶 |
| FP | mem[1] | 帧指针，指向当前栈帧底部 |
| TMP0 | mem[2] | 编译器临时变量 0 |
| TMP1 | mem[3] | 编译器临时变量 1 |
| TMP2 | mem[4] | 编译器临时变量 2 |
| TMP3 | mem[5] | 编译器临时变量 3 |
| TMP4 | mem[6] | 编译器临时变量 4 |
| TMP5 | mem[7] | 编译器临时变量 5 |

> **说明**：本机只有 PC、ACC、Z、N 四个真寄存器。SP、FP、TMP 是软件约定，把内存固定地址当作寄存器使用。模拟器状态只有 PC、ACC、Z、N、halted 五个变量。

---

## 三、内存布局

```
00000 - 00009 : 系统区（SP、FP、TMP0-5）
00010 - 00099 : 编译器临时区
00100 - 00999 : 全局变量区
01000 - 04999 : 栈区（从 4999 向下增长）
05000 - 09999 : 堆 / 数组区
10000 - 59999 : 代码区
60000 - 60015 : 内存映射 I/O 端口区
60016 - 65535 : 保留
```

> **说明**：这只是编译器约定，不是硬件强制。硬件不区分代码、数据和端口，只区分地址。

---

## 四、指令格式

### 4.1 编码格式

```
[4 位 n][12 位 subop] [opr1] [opr2] ...

n     = 操作数个数（0、1、2）
subop = 子操作码，每档独立编号
opr   = 16 位操作数（地址或立即数）
```

### 4.2 取指逻辑

```python
op = mem[pc]
n = (op >> 12) & 0xF
sub = op & 0xFFF
operands = mem[pc+1 : pc+1+n]
pc += 1 + n
```

### 4.3 指令长度

| 高 4 位 | 操作数个数 | 指令总长 |
|---------|-----------|----------|
| 0 | 0 | 1 字 |
| 1 | 1 | 2 字 |
| 2 | 2 | 3 字 |

---

## 五、指令表

### 5.1 0xxx 档：无操作数指令（5 条）

| 机器码 | 助记符 | 含义 | 影响标志 |
|--------|--------|------|----------|
| 0x0001 | HLT | 停机 | — |
| 0x0002 | NOP | 空操作 | — |
| 0x0005 | PUSH | mem[SP] = ACC; SP = SP - 1 | — |
| 0x0006 | POP | SP = SP + 1; ACC = mem[SP] | Z, N |
| 0x0007 | RET | 弹出返回地址到 PC | — |

### 5.2 1xxx 档：1 操作数指令（14 条）

| 机器码 | 助记符 | 含义 | 影响标志 |
|--------|--------|------|----------|
| 0x1001 [addr] | LDA addr | ACC = mem[addr] | Z, N |
| 0x1002 [addr] | STA addr | mem[addr] = ACC | — |
| 0x1003 [imm] | LDI imm | ACC = imm | Z, N |
| 0x1004 [addr] | ADD addr | ACC = ACC + mem[addr] | Z, N |
| 0x1005 [addr] | SUB addr | ACC = ACC - mem[addr] | Z, N |
| 0x1006 [addr] | AND addr | ACC = ACC & mem[addr] | Z, N |
| 0x1007 [addr] | OR addr | ACC = ACC \| mem[addr] | Z, N |
| 0x1008 [addr] | XOR addr | ACC = ACC ^ mem[addr] | Z, N |
| 0x1009 [addr] | BRA addr | PC = addr | — |
| 0x100A [addr] | BRZ addr | if Z: PC = addr | — |
| 0x100B [addr] | BLT addr | if N: PC = addr | — |
| 0x1011 [addr] | CALL addr | 压返回地址，PC = addr | — |
| 0x1012 [addr] | IN addr | ACC = 从端口 addr 读 | Z, N |
| 0x1013 [addr] | OUT addr | 把 ACC 写到端口 addr | — |

### 5.3 2xxx 档：2 操作数指令（2 条）

| 机器码 | 助记符 | 含义 | 影响标志 |
|--------|--------|------|----------|
| 0x2004 [a][b] | CMP a, b | 比较 mem[a] 和 mem[b]，设置 Z/N | Z, N |
| 0x2005 [port][imm] | OUTI port, imm | 把立即数 imm 写到端口 port | — |

### 5.4 指令统计

| 档位 | 条数 |
|------|------|
| 0xxx | 5 |
| 1xxx | 14 |
| 2xxx | 2 |
| **合计** | **21** |

---

## 六、标志位规则

| 指令 | Z 标志 | N 标志 |
|------|--------|--------|
| LDA / LDI / POP / IN | ACC == 0 | ACC < 0 |
| ADD / SUB / AND / OR / XOR | 结果 == 0 | 结果 < 0 |
| CMP a, b | mem[a] == mem[b] | mem[a] < mem[b] |
| STA / PUSH / 分支 / CALL / RET / OUT / OUTI / NOP / HLT | 不变 | 不变 |

---

## 七、栈与函数调用约定

### 7.1 栈指针

```
mem[0] = SP，初始值 = 4999（栈顶）
栈从高地址向低地址增长
```

### 7.2 PUSH / POP

```
PUSH：
    mem[SP] = ACC
    SP = SP - 1

POP：
    SP = SP + 1
    ACC = mem[SP]
```

### 7.3 CALL / RET

```
CALL addr：
    mem[SP] = PC
    SP = SP - 1
    PC = addr

RET：
    SP = SP + 1
    PC = mem[SP]
```

### 7.4 函数调用约定

- 参数由调用者压栈
- 返回值放 ACC
- 调用者负责清理参数
- 局部变量放栈帧，用 FP 寻址

---

## 八、内存映射 I/O

### 8.1 端口地址表

| 地址 | 方向 | 含义 |
|------|------|------|
| 60000 | 读/写 | 标准输入 / 标准输出 |
| 60001 | 读/写 | 字符输入 / 字符输出（ASCII） |
| 60002 | 读 | 随机数 |
| 60003 | 读 | 计时器（指令周期数） |
| 60004 | 读/写 | 文件读 / 文件写 |
| 60005 | 写 | 调试输出 |
| 60006 - 60015 | — | 保留 |

### 8.2 I/O 指令语义

```
IN addr：
    ACC = read_port(addr)
    addr 必须在端口区

OUT addr：
    write_port(addr, ACC)
    addr 必须在端口区

OUTI port, imm：
    write_port(port, imm)
    等价于 LDI imm + OUT port，但一条指令完成
```

### 8.3 模拟器实现

```python
PORT_BASE = 60000
PORT_END = 60015

def read_port(addr):
    if not (PORT_BASE <= addr <= PORT_END):
        raise ValueError(f"非法端口地址 {addr}")
    port = addr - PORT_BASE
    if port == 0:
        return int(input())
    elif port == 1:
        ch = sys.stdin.read(1)
        return ord(ch) if ch else 0
    elif port == 2:
        return random.randint(0, 65535)
    elif port == 3:
        return cycle_count
    elif port == 4:
        return read_file_next()
    else:
        raise ValueError(f"未定义输入端口 {port}")

def write_port(addr, value):
    if not (PORT_BASE <= addr <= PORT_END):
        raise ValueError(f"非法端口地址 {addr}")
    port = addr - PORT_BASE
    if port == 0:
        print(value)
    elif port == 1:
        sys.stdout.write(chr(value & 0xFFFF))
        sys.stdout.flush()
    elif port == 4:
        write_file_next(value)
    elif port == 5:
        print(f"[DEBUG] {value}")
    else:
        raise ValueError(f"未定义输出端口 {port}")
```

### 8.4 注意事项

- `IN` / `OUT` / `OUTI` 的地址必须是端口区地址，模拟器要检查范围
- 端口读写有副作用：读键盘会阻塞，写屏幕会输出
- 普通内存读写没有副作用，编译器要区分
- 端口地址和普通内存地址共享同一地址空间

---

## 九、指令速查

```
0xxx 档：
  HLT  NOP  PUSH  POP  RET

1xxx 档：
  LDA  STA  LDI  ADD  SUB  AND  OR  XOR
  BRA  BRZ  BLT  CALL
  IN   OUT

2xxx 档：
  CMP  OUTI
```

### 条件分支的等价写法

只有 `BRZ` 和 `BLT` 两条条件分支，其余条件用组合实现：

| 条件 | 实现 |
|------|------|
| `if a == b` | `CMP a, b` + `BRZ L` |
| `if a != b` | `CMP a, b` + `BRZ skip` + `BRA L` + `skip:` |
| `if a < b` | `CMP a, b` + `BLT L` |
| `if a >= b` | `CMP a, b` + `BLT skip` + `BRA L` + `skip:` |
| `if a <= b` | `CMP a, b` + `BLT L` + `BRZ L` |
| `if a > b` | `CMP a, b` + `BLT skip` + `BRZ skip` + `BRA L` + `skip:` |

---

## 十、代码示例

### 10.1 输出数字 42

```
LDI 42
OUT 60000
```

或：

```
OUTI 60000, 42
```

### 10.2 从键盘读一个数，加 1 后输出

```
IN 60000        ; ACC = 键盘输入
STA tmp0        ; 保存到临时变量
LDI 1           ; ACC = 1
ADD tmp0        ; ACC = 1 + 输入
OUT 60000       ; 输出结果
```

### 10.3 输出字符串 "Hi"

```
OUTI 60001, 72   ; 'H'
OUTI 60001, 105  ; 'i'
```

### 10.4 循环求和 1 到 10

```c
int i;
int sum;
sum = 0;
for (i = 1; i <= 10; i = i + 1) {
    sum = sum + i;
}
```

编译为 LMC 汇编：

```
        LDI 0
        STA sum         ; sum = 0
        LDI 1
        STA i           ; i = 1

loop:
        LDA i
        SUB ten         ; ten 里存常量 10
        BGE end         ; if i >= 10 跳出
        LDA sum
        ADD i
        STA sum         ; sum += i
        LDA i
        ADD one
        STA i           ; i += 1
        BRA loop

end:
        LDA sum
        OUT 60000
        HLT

i:      EQU 100
sum:    EQU 101
ten:    EQU 102
one:    EQU 103
```

### 10.5 函数调用

```c
int add(int x, int y) {
    return x + y;
}

int main() {
    return add(3, 4);
}
```

编译为 LMC 汇编：

```
; main
        LDI 3
        PUSH            ; 参数 x
        LDI 4
        PUSH            ; 参数 y
        CALL add
        POP             ; 清理参数 y
        POP             ; 清理参数 x
        OUT 60000
        HLT

; add
add:
        POP             ; 弹出 y
        STA tmp0
        POP             ; 弹出 x
        ADD tmp0        ; ACC = x + y
        RET
```

---

## 十一、设计取舍

| 决策 | 选择 | 原因 |
|------|------|------|
| 字长 | 16 位 | 地址空间够大，操作数够用 |
| 指令长度 | 变长（0/1/2 操作数） | 用高 4 位编码操作数个数，取指逻辑只需三行 |
| 操作码划分 | 高 4 位长度 + 低 12 位子操作码 | 规则统一，扩展新指令简单 |
| 运算寄存器 | 只有 ACC | 简化模拟器，表达式求值用临时内存 |
| 标志位 | Z / N | CMP + 条件分支需要 |
| 条件分支 | 只保留 BRZ / BLT | 其他条件用组合实现，节省指令数 |
| 位运算 | 保留 AND / OR / XOR | C 语言常用，且实现简单 |
| 乘法 | 不实现 | 用循环加法替代，保持指令集精简 |
| 间接寻址 | 不实现 | 第一版不支持数组和指针 |
| 函数调用 | PUSH / POP / CALL / RET | 支撑 C 函数 |
| I/O | 内存映射 + IN / OUT / OUTI | 端口数量无限，贴近真实 CPU |
| 端口立即数 | OUTI port, imm | 直接写常量到端口，省一条指令 |

---

## 附录：模拟器状态

```python
mem = [0] * 65536   # 内存
pc = 0              # 硬件寄存器
acc = 0             # 硬件寄存器
z = False           # 硬件标志
n = False           # 硬件标志
halted = False      # 停机标志
cycle_count = 0     # 指令周期计数
```

SP、FP、TMP0-5 都在 `mem[0]` 到 `mem[7]` 中。

---

**文档结束**
