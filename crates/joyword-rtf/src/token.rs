//! RTF 1.9 Tokenizer.

use crate::budget::{RtfBudget, RtfError};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RtfToken {
    GroupStart,
    GroupEnd,
    ControlWord { name: String, param: Option<i32> },
    Text(String),
}

pub struct RtfTokenizer<'a> {
    input: &'a [u8],
    pos: usize,
    budget: RtfBudget,
}

impl<'a> RtfTokenizer<'a> {
    pub fn new(input: &'a [u8], budget: RtfBudget) -> Self {
        Self {
            input,
            pos: 0,
            budget,
        }
    }

    pub fn next_token(&mut self) -> Result<Option<RtfToken>, RtfError> {
        while self.pos < self.input.len() {
            let b = self.input[self.pos];
            match b {
                b'{' => {
                    self.pos += 1;
                    return Ok(Some(RtfToken::GroupStart));
                }
                b'}' => {
                    self.pos += 1;
                    return Ok(Some(RtfToken::GroupEnd));
                }
                b'\\' => {
                    self.pos += 1;
                    return self.parse_escape();
                }
                b'\r' | b'\n' => {
                    // RTF ignores unescaped CR and LF
                    self.pos += 1;
                }
                _ => {
                    return self.parse_plain_text();
                }
            }
        }
        Ok(None)
    }

    fn parse_escape(&mut self) -> Result<Option<RtfToken>, RtfError> {
        if self.pos >= self.input.len() {
            return Err(RtfError::UnexpectedEof);
        }
        let b = self.input[self.pos];

        // Control symbols: \*, \{, \}, \\, \~ (non-breaking space), \- (optional hyphen)
        if !b.is_ascii_alphabetic() {
            self.pos += 1;
            if b == b'*' {
                return Ok(Some(RtfToken::ControlWord {
                    name: "*".to_string(),
                    param: None,
                }));
            }
            if b == b'\'' {
                // Hex byte \'xx
                if self.pos + 2 > self.input.len() {
                    return Err(RtfError::UnexpectedEof);
                }
                let hex_str = std::str::from_utf8(&self.input[self.pos..self.pos + 2])
                    .map_err(|_| RtfError::MalformedHexByte)?;
                let byte_val =
                    u8::from_str_radix(hex_str, 16).map_err(|_| RtfError::MalformedHexByte)?;
                self.pos += 2;
                // Return as text char (latin1/ascii range or buffered)
                return Ok(Some(RtfToken::Text((byte_val as char).to_string())));
            }
            let ch = match b {
                b'{' => '{',
                b'}' => '}',
                b'\\' => '\\',
                b'~' => ' ',
                _ => b as char,
            };
            return Ok(Some(RtfToken::Text(ch.to_string())));
        }

        // Control word: letters followed by optional signed integer
        let start = self.pos;
        while self.pos < self.input.len() && self.input[self.pos].is_ascii_alphabetic() {
            self.pos += 1;
            if self.pos - start > self.budget.max_control_word_len {
                return Err(RtfError::BudgetExceeded("control word too long"));
            }
        }
        let name = std::str::from_utf8(&self.input[start..self.pos])
            .unwrap_or("")
            .to_string();

        let mut param = None;
        let num_start = self.pos;
        let mut has_digit = false;
        if self.pos < self.input.len()
            && (self.input[self.pos] == b'-' || self.input[self.pos].is_ascii_digit())
        {
            if self.input[self.pos] == b'-' {
                self.pos += 1;
            }
            while self.pos < self.input.len() && self.input[self.pos].is_ascii_digit() {
                self.pos += 1;
                has_digit = true;
            }
            if has_digit && let Ok(num_str) = std::str::from_utf8(&self.input[num_start..self.pos])
            {
                param = num_str.parse::<i32>().ok();
            }
        }

        // A single space following a control word is a delimiter and consumed
        if self.pos < self.input.len() && self.input[self.pos] == b' ' {
            self.pos += 1;
        }

        Ok(Some(RtfToken::ControlWord { name, param }))
    }

    fn parse_plain_text(&mut self) -> Result<Option<RtfToken>, RtfError> {
        let start = self.pos;
        while self.pos < self.input.len() {
            let b = self.input[self.pos];
            if b == b'{' || b == b'}' || b == b'\\' || b == b'\r' || b == b'\n' {
                break;
            }
            self.pos += 1;
        }
        let slice = &self.input[start..self.pos];
        let text = String::from_utf8_lossy(slice).into_owned();
        Ok(Some(RtfToken::Text(text)))
    }
}
