//! Scalar encodings shared by the BIFF8 and BIFF12 workbook record readers.
//! [MS-XLS] RkNumber and BErr, [MS-XLSB] RkNumber and BErr.
use joysheet_core::CellError;

/// Bit 1 chooses a signed 30-bit integer or the high 30 bits of a binary64.
/// Bit 0 restores two implied decimal places in either representation.
pub(super) fn rk_number(record: u32) -> f64 {
    let payload = record >> 2;
    let number = if record & 2 == 0 { f64::from_bits(u64::from(payload) << 34) } else { f64::from((record as i32) >> 2) };
    if record & 1 == 0 { number } else { number / 100.0 }
}

pub(super) fn error_value(code: u8) -> Option<CellError> {
    match code {
        0x00 => Some(CellError::Null),
        0x07 => Some(CellError::Div0),
        0x0f => Some(CellError::Value),
        0x17 => Some(CellError::Ref),
        0x1d => Some(CellError::Name),
        0x24 => Some(CellError::Num),
        0x2a => Some(CellError::NA),
        0x2b => Some(CellError::GettingData),
        _ => None,
    }
}
