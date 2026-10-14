//! 极小 JSON 写出器（避免为实验引入 serde 依赖）。只写对象/数组/字符串/
//! 数字/布尔；字符串转义覆盖 JSON 规范要求；非 ASCII 原样输出 UTF-8。

pub fn string(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 2);
    out.push('"');
    for character in text.chars() {
        match character {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            control if (control as u32) < 0x20 => {
                out.push_str(&format!("\\u{:04x}", control as u32));
            }
            other => out.push(other),
        }
    }
    out.push('"');
    out
}

/// 逐行辅助：`indent` 为每层两个空格；调用方自行拼键值对。
pub fn field(name: &str, value: &str, indent: usize) -> String {
    format!("{}{}: {}", "  ".repeat(indent), string(name), value)
}
