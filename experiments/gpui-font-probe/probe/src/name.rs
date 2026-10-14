//! `name` 表改写器（W02 探针原创实现，依据公开规范，未搬运任何编辑器实现）。
//!
//! 依据：
//! - OpenType 规范 Table Directory 与 TTC Header：
//!   <https://learn.microsoft.com/en-us/typography/opentype/spec/otff>
//! - OpenType 规范 `name` 表（format 0）：
//!   <https://learn.microsoft.com/en-us/typography/opentype/spec/name>
//!
//! 目标：给定一个字体文件（可为 TTC 集合）与 face 序号，把该 face 的
//! 家族/子族/全名/PostScript 名（nameID 1、2、4、6、16、17，全部平台）
//! 替换为调用方给定的新名字，其余记录与表保持字节不变。新 `name` 表
//! 追加在文件末尾（原表成为死空间，不影响其他 face 的任何偏移），
//! 并更新该 face 表目录中 `name` 项的偏移/长度/校验和与 `head` 的
//! checkSumAdjustment。
//!
//! 改写只影响元数据；glyf/cmap/GSUB 等与整形相关的表不动，因此同一
//! 输入文本整形结果必须不变——这一点由测试 `chain.rs` 机器验证。

/// 替换的 nameID：家族、子族、全名、PostScript 名、排版家族、排版子族。
const RENAMED_IDS: [u16; 6] = [1, 2, 4, 6, 16, 17];

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NewNames {
    pub family: String,
    pub subfamily: String,
    pub full: String,
    pub postscript: String,
}

#[derive(Clone, Debug)]
struct NameRecord {
    platform_id: u16,
    encoding_id: u16,
    language_id: u16,
    name_id: u16,
    string: Vec<u8>,
}

#[derive(Debug)]
pub struct RenameError(pub String);

impl std::fmt::Display for RenameError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "name rewrite failed: {}", self.0)
    }
}

fn err(message: impl Into<String>) -> RenameError {
    RenameError(message.into())
}

fn u16_at(data: &[u8], offset: usize) -> Result<u16, RenameError> {
    let bytes = data
        .get(offset..offset + 2)
        .ok_or_else(|| err("truncated u16"))?;
    Ok(u16::from_be_bytes([bytes[0], bytes[1]]))
}

fn u32_at(data: &[u8], offset: usize) -> Result<u32, RenameError> {
    let bytes = data
        .get(offset..offset + 4)
        .ok_or_else(|| err("truncated u32"))?;
    Ok(u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
}

/// TTC 集合中的 face 数；单字体文件返回 1。
pub fn face_count(data: &[u8]) -> Result<u32, RenameError> {
    if data.starts_with(b"ttcf") {
        Ok(u32_at(data, 8)?)
    } else if valid_sfnt_tag(data).is_some() {
        Ok(1)
    } else {
        Err(err("not an OTF/TTF/TTC file"))
    }
}

/// 返回 sfnt 标签（`OTTO`、`0x00010000`、`true`、`ttcf` 之外的校验）。
fn valid_sfnt_tag(data: &[u8]) -> Option<()> {
    let tag = data.get(..4)?;
    if tag == b"OTTO" || tag == b"true" || tag == b"\0\x01\0\0" || tag == b"ttcf" {
        Some(())
    } else {
        None
    }
}

/// face `index` 在文件内的字节偏移（TTC 查 OffsetTable；单字体必须为 0 号）。
fn face_offset(data: &[u8], index: u32) -> Result<usize, RenameError> {
    if !valid_sfnt_tag(data).is_some() {
        return Err(err("unknown sfnt tag"));
    }
    if data.starts_with(b"ttcf") {
        let count = u32_at(data, 8)?;
        if index >= count {
            return Err(err(format!("face index {index} out of range ({count})")));
        }
        let offset = u32_at(data, 12 + 4 * index as usize)?;
        usize::try_from(offset).map_err(|_| err("offset overflow"))
    } else if index == 0 {
        Ok(0)
    } else {
        Err(err("single font has no face 1+"))
    }
}

struct TableEntry {
    tag: [u8; 4],
    checksum: u32,
    offset: u32,
    length: u32,
}

fn read_table_directory(data: &[u8], face_off: usize) -> Result<Vec<TableEntry>, RenameError> {
    let num_tables = u16_at(data, face_off + 4)? as usize;
    let mut entries = Vec::with_capacity(num_tables);
    for index in 0..num_tables {
        let at = face_off + 12 + 16 * index;
        let bytes = data
            .get(at..at + 16)
            .ok_or_else(|| err("truncated table directory"))?;
        entries.push(TableEntry {
            tag: [bytes[0], bytes[1], bytes[2], bytes[3]],
            checksum: u32::from_be_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]),
            offset: u32::from_be_bytes([bytes[8], bytes[9], bytes[10], bytes[11]]),
            length: u32::from_be_bytes([bytes[12], bytes[13], bytes[14], bytes[15]]),
        });
    }
    Ok(entries)
}

/// 规范校验和：按大端 u32 求和，末尾不足 4 字节按 0 补齐。
fn checksum(data: &[u8]) -> u32 {
    let mut sum: u32 = 0;
    let (chunks, remainder) = data.as_chunks::<4>();
    for chunk in chunks {
        sum = sum.wrapping_add(u32::from_be_bytes(*chunk));
    }
    if !remainder.is_empty() {
        let mut last = [0_u8; 4];
        last[..remainder.len()].copy_from_slice(remainder);
        sum = sum.wrapping_add(u32::from_be_bytes(last));
    }
    sum
}

fn encode_string(platform_id: u16, text: &str) -> Result<Vec<u8>, RenameError> {
    match platform_id {
        // Unicode 与 Windows 平台：UTF-16BE。
        0 | 3 => Ok(text.encode_utf16().flat_map(u16::to_be_bytes).collect()),
        // Macintosh 平台：ASCII（本探针生成的新名字均为 ASCII）。
        1 => {
            let bytes = text.as_bytes();
            if bytes.is_ascii() {
                Ok(bytes.to_vec())
            } else {
                Err(err("mac platform strings must stay ASCII"))
            }
        }
        other => Err(err(format!("unsupported platform {other}"))),
    }
}

fn replacement(names: &NewNames, name_id: u16) -> Option<&str> {
    match name_id {
        1 | 16 => Some(&names.family),
        2 | 17 => Some(&names.subfamily),
        4 => Some(&names.full),
        6 => Some(&names.postscript),
        _ => None,
    }
}

/// 序列化新的 format 0 `name` 表。
fn build_name_table(records: &[NameRecord]) -> Vec<u8> {
    let count = records.len() as u16;
    let string_offset = 6 + 12 * records.len();
    let mut table =
        Vec::with_capacity(string_offset + records.iter().map(|r| r.string.len()).sum::<usize>());
    table.extend_from_slice(&0u16.to_be_bytes()); // format
    table.extend_from_slice(&count.to_be_bytes());
    table.extend_from_slice(&(string_offset as u16).to_be_bytes());
    let mut storage = Vec::new();
    for record in records {
        let offset = storage.len() as u16;
        storage.extend_from_slice(&record.string);
        table.extend_from_slice(&record.platform_id.to_be_bytes());
        table.extend_from_slice(&record.encoding_id.to_be_bytes());
        table.extend_from_slice(&record.language_id.to_be_bytes());
        table.extend_from_slice(&record.name_id.to_be_bytes());
        table.extend_from_slice(&(record.string.len() as u16).to_be_bytes());
        table.extend_from_slice(&offset.to_be_bytes());
    }
    table.extend_from_slice(&storage);
    // 表长度按 4 字节对齐，便于校验和计算与消费端读取。
    while table.len() % 4 != 0 {
        table.push(0);
    }
    table
}

/// 列出某个 face 的全部表标签（供逐表一致性验证）。
pub fn table_tags(data: &[u8], face_index: u32) -> Result<Vec<[u8; 4]>, RenameError> {
    let face_off = face_offset(data, face_index)?;
    Ok(read_table_directory(data, face_off)?
        .into_iter()
        .map(|entry| entry.tag)
        .collect())
}

/// 取出某个 face 的全部表，重打包为独立单字体文件。表内容字节保持
/// 不变（只重排偏移）；目录项的标签/长度/校验和沿用原值，最后重算
/// head 的 checkSumAdjustment。
///
/// 原因：gpui 0.2.2 的 macOS `add_fonts` 通过 `CGFont::from_data_provider`
/// 注册内存字体，CGFont 对 TTC 集合只会装载 face 0；要注册指定 face
/// 必须先拆成单字体文件（见报告）。
pub fn extract_face(data: &[u8], face_index: u32) -> Result<Vec<u8>, RenameError> {
    let face_off = face_offset(data, face_index)?;
    let entries = read_table_directory(data, face_off)?;
    let mut out = Vec::new();
    // 头 12 字节（sfntVersion/numTables/searchRange/entrySelector/
    // rangeShift）照抄：numTables 未变，派生字段仍有效。
    out.extend_from_slice(
        data.get(face_off..face_off + 12)
            .ok_or_else(|| err("truncated face header"))?,
    );
    let directory_size = 12 + 16 * entries.len();
    out.resize(directory_size, 0);
    let mut new_offsets = Vec::with_capacity(entries.len());
    for entry in &entries {
        let table = data
            .get(entry.offset as usize..(entry.offset + entry.length) as usize)
            .ok_or_else(|| err("table out of bounds"))?;
        while out.len() % 4 != 0 {
            out.push(0);
        }
        new_offsets.push(out.len() as u32);
        out.extend_from_slice(table);
    }
    for (index, entry) in entries.iter().enumerate() {
        let at = 12 + 16 * index;
        out[at..at + 4].copy_from_slice(&entry.tag);
        out[at + 4..at + 8].copy_from_slice(&entry.checksum.to_be_bytes());
        out[at + 8..at + 12].copy_from_slice(&new_offsets[index].to_be_bytes());
        out[at + 12..at + 16].copy_from_slice(&entry.length.to_be_bytes());
    }
    update_head_adjustment(&mut out, 0)?;
    Ok(out)
}

/// 读某个 face 一张表的原始字节（供表级一致性验证）。
pub fn table_bytes(data: &[u8], face_index: u32, tag: &[u8; 4]) -> Result<Vec<u8>, RenameError> {
    let face_off = face_offset(data, face_index)?;
    let entries = read_table_directory(data, face_off)?;
    let entry = entries
        .iter()
        .find(|entry| &entry.tag == tag)
        .ok_or_else(|| err("no such table"))?;
    data.get(entry.offset as usize..(entry.offset + entry.length) as usize)
        .map(|bytes| bytes.to_vec())
        .ok_or_else(|| err("table out of bounds"))
}

/// 把 `data` 中 `face_index` 号 face 的名称记录改写为 `names`，返回新文件。
///
/// 新 `name` 表追加到文件末尾（4 字节对齐）；该 face 表目录的 `name`
/// 项与 `head` 的 checkSumAdjustment 同步更新。TTC 中其他 face 的所有
/// 表偏移不受影响。
pub fn rename_face(data: &[u8], face_index: u32, names: &NewNames) -> Result<Vec<u8>, RenameError> {
    for value in [
        &names.family,
        &names.subfamily,
        &names.full,
        &names.postscript,
    ] {
        if !value.is_ascii() {
            return Err(err("new names must be ASCII for the mac platform"));
        }
    }
    let face_off = face_offset(data, face_index)?;
    let entries = read_table_directory(data, face_off)?;
    let name_entry = entries
        .iter()
        .find(|entry| &entry.tag == b"name")
        .ok_or_else(|| err("no name table"))?;
    let name_off = name_entry.offset as usize;
    let name_len = name_entry.length as usize;
    let table = data
        .get(name_off..name_off + name_len)
        .ok_or_else(|| err("name table out of bounds"))?;
    let format = u16_at(table, 0)?;
    if format != 0 {
        return Err(err(format!("unsupported name table format {format}")));
    }
    let count = u16_at(table, 2)? as usize;
    let string_offset = u16_at(table, 4)? as usize;
    let mut records = Vec::with_capacity(count);
    for index in 0..count {
        let at = 6 + 12 * index;
        let platform_id = u16_at(table, at)?;
        let encoding_id = u16_at(table, at + 2)?;
        let language_id = u16_at(table, at + 4)?;
        let name_id = u16_at(table, at + 6)?;
        let length = u16_at(table, at + 8)? as usize;
        let offset = u16_at(table, at + 10)? as usize;
        let string = table
            .get(string_offset + offset..string_offset + offset + length)
            .ok_or_else(|| err("name string out of bounds"))?;
        let string = if RENAMED_IDS.contains(&name_id) {
            encode_string(
                platform_id,
                replacement(names, name_id).expect("id checked above"),
            )?
        } else {
            string.to_vec()
        };
        records.push(NameRecord {
            platform_id,
            encoding_id,
            language_id,
            name_id,
            string,
        });
    }

    let new_table = build_name_table(&records);
    let mut out = data.to_vec();
    while !out.len().is_multiple_of(4) {
        out.push(0);
    }
    let new_name_off = out.len() as u32;
    out.extend_from_slice(&new_table);

    // 更新该 face 表目录中的 name 项（偏移、长度、校验和）。
    let num_tables = u16_at(data, face_off + 4)? as usize;
    let mut name_dir_at = None;
    for index in 0..num_tables {
        let at = face_off + 12 + 16 * index;
        if &data[at..at + 4] == b"name" {
            name_dir_at = Some(at);
            break;
        }
    }
    let at = name_dir_at.ok_or_else(|| err("no name directory entry"))?;
    let name_checksum = checksum(&new_table);
    out[at + 4..at + 8].copy_from_slice(&name_checksum.to_be_bytes());
    out[at + 8..at + 12].copy_from_slice(&new_name_off.to_be_bytes());
    out[at + 12..at + 16].copy_from_slice(&(new_table.len() as u32).to_be_bytes());

    update_head_adjustment(&mut out, face_off)?;
    Ok(out)
}

/// `head.checkSumAdjustment = 0xB1B0AFBA - sum(whole file with the field
/// zeroed)`。TTC 情形下规范未严格要求，消费端也不校验；这里按单字体
/// 算法对整个文件计算并如实记录（见报告）。
fn update_head_adjustment(out: &mut [u8], face_off: usize) -> Result<(), RenameError> {
    let entries = read_table_directory(out, face_off)?;
    let head = entries
        .iter()
        .find(|entry| &entry.tag == b"head")
        .ok_or_else(|| err("no head table"))?;
    let adjustment_at = head.offset as usize + 8;
    if adjustment_at + 4 > out.len() {
        return Err(err("head table truncated"));
    }
    out[adjustment_at..adjustment_at + 4].copy_from_slice(&0u32.to_be_bytes());
    // 校验和按 4 字节对齐计算；文件不足处按 0 补齐。
    let pad = (4 - out.len() % 4) % 4;
    let mut padded = Vec::with_capacity(out.len() + pad);
    padded.extend_from_slice(out);
    padded.resize(out.len() + pad, 0);
    let adjustment = 0xB1B0_AFBA_u32.wrapping_sub(checksum(&padded));
    out[adjustment_at..adjustment_at + 4].copy_from_slice(&adjustment.to_be_bytes());

    // 表目录中 head 项的校验和按"adjustment 置零"的表内容计算。
    let head_off = head.offset as usize;
    let head_len = head.length as usize;
    let mut head_bytes = out[head_off..head_off + head_len].to_vec();
    while !head_bytes.len().is_multiple_of(4) {
        head_bytes.push(0);
    }
    head_bytes[8..12].copy_from_slice(&0u32.to_be_bytes());
    let num_tables = entries.len();
    for index in 0..num_tables {
        let at = face_off + 12 + 16 * index;
        if &out[at..at + 4] == b"head" {
            let head_checksum = checksum(&head_bytes);
            out[at + 4..at + 8].copy_from_slice(&head_checksum.to_be_bytes());
            break;
        }
    }
    Ok(())
}

/// 读回某个 face 的名称字符串（platform 3 Windows 或 platform 1 Mac），
/// 供验证与记录使用。返回 (name_id, platform_id, language_id, text)。
pub fn read_names(
    data: &[u8],
    face_index: u32,
) -> Result<Vec<(u16, u16, u16, String)>, RenameError> {
    let face_off = face_offset(data, face_index)?;
    let entries = read_table_directory(data, face_off)?;
    let name_entry = entries
        .iter()
        .find(|entry| &entry.tag == b"name")
        .ok_or_else(|| err("no name table"))?;
    let name_off = name_entry.offset as usize;
    let name_len = name_entry.length as usize;
    let table = data
        .get(name_off..name_off + name_len)
        .ok_or_else(|| err("name table out of bounds"))?;
    let count = u16_at(table, 2)? as usize;
    let string_offset = u16_at(table, 4)? as usize;
    let mut result = Vec::with_capacity(count);
    for index in 0..count {
        let at = 6 + 12 * index;
        let platform_id = u16_at(table, at)?;
        let language_id = u16_at(table, at + 4)?;
        let name_id = u16_at(table, at + 6)?;
        let length = u16_at(table, at + 8)? as usize;
        let offset = u16_at(table, at + 10)? as usize;
        let string = table
            .get(string_offset + offset..string_offset + offset + length)
            .ok_or_else(|| err("name string out of bounds"))?;
        let text = match platform_id {
            0 | 3 => {
                let (pairs, tail) = string.as_chunks::<2>();
                if !tail.is_empty() {
                    return Err(err("odd-length utf16 string"));
                }
                let units: Vec<u16> = pairs.iter().map(|pair| u16::from_be_bytes(*pair)).collect();
                String::from_utf16(&units).map_err(|_| err("bad utf16"))?
            }
            1 => {
                // Mac Roman 与 ASCII 前缀兼容；此解码仅供读取验证/记录，
                // 改写输出对未命中记录原样保留字节，不引入有损转换。
                String::from_utf8_lossy(string).into_owned()
            }
            _ => continue,
        };
        result.push((name_id, platform_id, language_id, text));
    }
    Ok(result)
}
