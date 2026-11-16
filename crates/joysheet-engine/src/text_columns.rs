//! Shared, bounded-by-caller text-to-columns parsing for editing and desktop preview.
use serde_json::Value;

/// Numeric date orders exposed by the text import wizard. Invalid dates stay text.
pub fn date_value(text: &str, order: &str, system: joysheet_core::date::DateSystem) -> Option<f64> {
    let order = order.strip_prefix("date")?;
    let parts = text.trim().split(['/', '-', '.', ' ']).filter(|p| !p.is_empty()).map(str::parse::<i64>).collect::<Result<Vec<_>, _>>().ok()?;
    if parts.len() != 3 {
        return None;
    }
    let mut y = 0;
    let mut m = 0;
    let mut d = 0;
    for (c, v) in order.chars().zip(parts) {
        match c {
            'Y' => y = v,
            'M' => m = v,
            'D' => d = v,
            _ => return None,
        }
    }
    if y < 100 {
        y += if y < 30 { 2000 } else { 1900 };
    }
    joysheet_core::date::serial_from_ymd(system, y, m, d)
}

pub fn split(text: &str, p: &Value) -> Vec<String> {
    if let Some(widths) = p.get("fixedWidths").and_then(Value::as_array) {
        let chars: Vec<char> = text.chars().collect();
        let mut cuts = vec![0];
        cuts.extend(widths.iter().filter_map(Value::as_u64).filter_map(|v| usize::try_from(v).ok()).filter(|&v| v > 0 && v < chars.len()));
        cuts.sort_unstable();
        cuts.dedup();
        cuts.push(chars.len());
        return cuts.windows(2).map(|c| chars[c[0]..c[1]].iter().collect::<String>().trim().to_string()).collect();
    }
    let mut delims: Vec<char> = p
        .get("delimiters")
        .and_then(Value::as_array)
        .map(|a| a.iter().filter_map(|d| d.as_str()?.chars().next()).collect())
        .unwrap_or_else(|| vec!['\t']);
    if let Some(c) = p.get("other").and_then(Value::as_str).and_then(|s| s.chars().next()) {
        delims.push(c);
    }
    split_delimited(
        text,
        &delims,
        p["treatConsecutive"].as_bool().unwrap_or(false),
        p.get("textQualifier").and_then(Value::as_str).map(|s| s.chars().next().unwrap_or('\0')).unwrap_or('"'),
    )
}
pub fn split_delimited(text: &str, delims: &[char], consecutive: bool, quote: char) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut in_q = false;
    let mut last_delim = false;
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if c == quote {
            if in_q && chars.peek() == Some(&quote) {
                cur.push(quote);
                chars.next();
            } else {
                in_q = !in_q;
            }
            continue;
        }
        if !in_q && delims.contains(&c) {
            if consecutive && last_delim {
                continue;
            }
            out.push(std::mem::take(&mut cur));
            last_delim = true;
            continue;
        }
        last_delim = false;
        cur.push(c);
    }
    out.push(cur);
    out
}
