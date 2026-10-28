//! Office Math Markup Language (OMML) subset and linear math mapping (ECMA-376 Part 1 §22).

use quick_xml::events::{BytesStart, Event};
use quick_xml::reader::Reader;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OmmlBudget {
    pub max_depth: usize,
    pub max_nodes: usize,
}

impl Default for OmmlBudget {
    fn default() -> Self {
        Self {
            max_depth: 64,
            max_nodes: 4096,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OmmlError {
    Xml(String),
    BudgetExceeded(String),
    Malformed(String),
}

impl std::fmt::Display for OmmlError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Xml(e) => write!(f, "XML parsing error: {e}"),
            Self::BudgetExceeded(e) => write!(f, "OMML budget exceeded: {e}"),
            Self::Malformed(e) => write!(f, "Malformed OMML: {e}"),
        }
    }
}

impl std::error::Error for OmmlError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MathElement {
    Text(String),
    Fraction {
        num: Box<MathElement>,
        den: Box<MathElement>,
    },
    Superscript {
        base: Box<MathElement>,
        sup: Box<MathElement>,
    },
    Subscript {
        base: Box<MathElement>,
        sub: Box<MathElement>,
    },
    SubSup {
        base: Box<MathElement>,
        sub: Box<MathElement>,
        sup: Box<MathElement>,
    },
    Radical {
        base: Box<MathElement>,
        deg: Option<Box<MathElement>>,
    },
    Delimiter {
        open: char,
        close: char,
        content: Vec<MathElement>,
    },
    Group(Vec<MathElement>),
}

impl MathElement {
    /// Format element into linear mathematical notation (e.g. `(a)/(b)`, `x^2`, `\sqrt(x)`).
    pub fn to_linear_text(&self) -> String {
        match self {
            Self::Text(s) => s.clone(),
            Self::Fraction { num, den } => {
                format!("({})/({})", num.to_linear_text(), den.to_linear_text())
            }
            Self::Superscript { base, sup } => {
                format!("({})^({})", base.to_linear_text(), sup.to_linear_text())
            }
            Self::Subscript { base, sub } => {
                format!("({})_({})", base.to_linear_text(), sub.to_linear_text())
            }
            Self::SubSup { base, sub, sup } => {
                format!(
                    "({})_({})^({})",
                    base.to_linear_text(),
                    sub.to_linear_text(),
                    sup.to_linear_text()
                )
            }
            Self::Radical { base, deg } => {
                if let Some(d) = deg {
                    format!(
                        "\\sqrt[{}]{{({})}}",
                        d.to_linear_text(),
                        base.to_linear_text()
                    )
                } else {
                    format!("\\sqrt{{{}}}", base.to_linear_text())
                }
            }
            Self::Delimiter {
                open,
                close,
                content,
            } => {
                let inner: String = content.iter().map(MathElement::to_linear_text).collect();
                format!("{open}{inner}{close}")
            }
            Self::Group(items) => items.iter().map(MathElement::to_linear_text).collect(),
        }
    }

    /// Serialize into OMML XML string.
    pub fn to_omml_xml(&self) -> String {
        let mut out = String::new();
        self.write_omml_body(&mut out);
        out
    }

    fn write_omml_body(&self, out: &mut String) {
        match self {
            Self::Text(s) => {
                out.push_str("<m:r><m:t>");
                out.push_str(&quick_xml::escape::escape(s));
                out.push_str("</m:t></m:r>");
            }
            Self::Fraction { num, den } => {
                out.push_str("<m:f><m:num>");
                num.write_omml_body(out);
                out.push_str("</m:num><m:den>");
                den.write_omml_body(out);
                out.push_str("</m:den></m:f>");
            }
            Self::Superscript { base, sup } => {
                out.push_str("<m:sSup><m:e>");
                base.write_omml_body(out);
                out.push_str("</m:e><m:sup>");
                sup.write_omml_body(out);
                out.push_str("</m:sup></m:sSup>");
            }
            Self::Subscript { base, sub } => {
                out.push_str("<m:sSub><m:e>");
                base.write_omml_body(out);
                out.push_str("</m:e><m:sub>");
                sub.write_omml_body(out);
                out.push_str("</m:sub></m:sSub>");
            }
            Self::SubSup { base, sub, sup } => {
                out.push_str("<m:sSubSup><m:e>");
                base.write_omml_body(out);
                out.push_str("</m:e><m:sub>");
                sub.write_omml_body(out);
                out.push_str("</m:sub><m:sup>");
                sup.write_omml_body(out);
                out.push_str("</m:sup></m:sSubSup>");
            }
            Self::Radical { base, deg } => {
                out.push_str("<m:rad><m:radPr>");
                if deg.is_none() {
                    out.push_str("<m:degHide m:val=\"on\"/>");
                }
                out.push_str("</m:radPr><m:deg>");
                if let Some(d) = deg {
                    d.write_omml_body(out);
                }
                out.push_str("</m:deg><m:e>");
                base.write_omml_body(out);
                out.push_str("</m:e></m:rad>");
            }
            Self::Delimiter {
                open,
                close,
                content,
            } => {
                out.push_str("<m:d><m:dPr>");
                out.push_str(&format!(
                    "<m:begChr m:val=\"{open}\"/><m:endChr m:val=\"{close}\"/>"
                ));
                out.push_str("</m:dPr><m:e>");
                for item in content {
                    item.write_omml_body(out);
                }
                out.push_str("</m:e></m:d>");
            }
            Self::Group(items) => {
                for item in items {
                    item.write_omml_body(out);
                }
            }
        }
    }
}

/// Parse OMML XML string into MathElement AST tree under budget control.
pub fn parse_omml_xml(xml: &str, budget: &OmmlBudget) -> Result<MathElement, OmmlError> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);

    let mut node_count = 0;
    let mut elements = Vec::new();

    loop {
        match reader.read_event() {
            Ok(Event::Start(ref e)) => {
                let name = e.name();
                let local = local_name(name.as_ref());
                if local == b"oMath" || local == b"oMathPara" {
                    let elem = parse_element(&mut reader, e, 1, &mut node_count, budget)?;
                    elements.push(elem);
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(OmmlError::Xml(e.to_string())),
            _ => {}
        }
    }

    if elements.len() == 1 {
        Ok(elements.pop().unwrap())
    } else {
        Ok(MathElement::Group(elements))
    }
}

fn parse_element(
    reader: &mut Reader<&[u8]>,
    start: &BytesStart<'_>,
    depth: usize,
    node_count: &mut usize,
    budget: &OmmlBudget,
) -> Result<MathElement, OmmlError> {
    *node_count += 1;
    if *node_count > budget.max_nodes {
        return Err(OmmlError::BudgetExceeded(format!(
            "node count {} exceeded limit {}",
            *node_count, budget.max_nodes
        )));
    }
    if depth > budget.max_depth {
        return Err(OmmlError::BudgetExceeded(format!(
            "depth {} exceeded limit {}",
            depth, budget.max_depth
        )));
    }

    let start_name = start.name();
    let tag_name = local_name(start_name.as_ref());

    match tag_name {
        b"oMath" | b"oMathPara" | b"e" | b"num" | b"den" | b"sub" | b"sup" | b"deg" => {
            let mut children = Vec::new();
            let end_name = start.name().as_ref().to_vec();
            loop {
                match reader.read_event() {
                    Ok(Event::Start(ref child)) => {
                        let child_elem =
                            parse_element(reader, child, depth + 1, node_count, budget)?;
                        children.push(child_elem);
                    }
                    Ok(Event::Text(ref t)) => {
                        let s = std::str::from_utf8(t.as_ref())
                            .map_err(|e| OmmlError::Xml(e.to_string()))?;
                        if !s.is_empty() {
                            children.push(MathElement::Text(s.to_string()));
                        }
                    }
                    Ok(Event::End(ref e)) if e.name().as_ref() == end_name => break,
                    Ok(Event::Eof) => break,
                    Err(e) => return Err(OmmlError::Xml(e.to_string())),
                    _ => {}
                }
            }
            if children.len() == 1 {
                Ok(children.pop().unwrap())
            } else {
                Ok(MathElement::Group(children))
            }
        }
        b"r" => {
            // Math run containing m:t
            let mut text = String::new();
            let end_name = start.name().as_ref().to_vec();
            loop {
                match reader.read_event() {
                    Ok(Event::Text(ref t)) => {
                        let s = std::str::from_utf8(t.as_ref())
                            .map_err(|e| OmmlError::Xml(e.to_string()))?;
                        text.push_str(s);
                    }
                    Ok(Event::End(ref e)) if e.name().as_ref() == end_name => break,
                    Ok(Event::Eof) => break,
                    Err(e) => return Err(OmmlError::Xml(e.to_string())),
                    _ => {}
                }
            }
            Ok(MathElement::Text(text))
        }
        b"t" => {
            let mut text = String::new();
            let end_name = start.name().as_ref().to_vec();
            loop {
                match reader.read_event() {
                    Ok(Event::Text(ref t)) => {
                        let s = std::str::from_utf8(t.as_ref())
                            .map_err(|e| OmmlError::Xml(e.to_string()))?;
                        text.push_str(s);
                    }
                    Ok(Event::End(ref e)) if e.name().as_ref() == end_name => break,
                    Ok(Event::Eof) => break,
                    Err(e) => return Err(OmmlError::Xml(e.to_string())),
                    _ => {}
                }
            }
            Ok(MathElement::Text(text))
        }
        b"f" => {
            let mut num = None;
            let mut den = None;
            let end_name = start.name().as_ref().to_vec();
            loop {
                match reader.read_event() {
                    Ok(Event::Start(ref child)) => {
                        let c_name = child.name();
                        let cname = local_name(c_name.as_ref());
                        if cname == b"num" {
                            num =
                                Some(parse_element(reader, child, depth + 1, node_count, budget)?);
                        } else if cname == b"den" {
                            den =
                                Some(parse_element(reader, child, depth + 1, node_count, budget)?);
                        } else {
                            skip_element(reader, child)?;
                        }
                    }
                    Ok(Event::End(ref e)) if e.name().as_ref() == end_name => break,
                    Ok(Event::Eof) => break,
                    Err(e) => return Err(OmmlError::Xml(e.to_string())),
                    _ => {}
                }
            }
            Ok(MathElement::Fraction {
                num: Box::new(num.unwrap_or(MathElement::Text(String::new()))),
                den: Box::new(den.unwrap_or(MathElement::Text(String::new()))),
            })
        }
        b"sSup" => {
            let mut base = None;
            let mut sup = None;
            let end_name = start.name().as_ref().to_vec();
            loop {
                match reader.read_event() {
                    Ok(Event::Start(ref child)) => {
                        let c_name = child.name();
                        let cname = local_name(c_name.as_ref());
                        if cname == b"e" {
                            base =
                                Some(parse_element(reader, child, depth + 1, node_count, budget)?);
                        } else if cname == b"sup" {
                            sup =
                                Some(parse_element(reader, child, depth + 1, node_count, budget)?);
                        } else {
                            skip_element(reader, child)?;
                        }
                    }
                    Ok(Event::End(ref e)) if e.name().as_ref() == end_name => break,
                    Ok(Event::Eof) => break,
                    Err(e) => return Err(OmmlError::Xml(e.to_string())),
                    _ => {}
                }
            }
            Ok(MathElement::Superscript {
                base: Box::new(base.unwrap_or(MathElement::Text(String::new()))),
                sup: Box::new(sup.unwrap_or(MathElement::Text(String::new()))),
            })
        }
        b"sSub" => {
            let mut base = None;
            let mut sub = None;
            let end_name = start.name().as_ref().to_vec();
            loop {
                match reader.read_event() {
                    Ok(Event::Start(ref child)) => {
                        let c_name = child.name();
                        let cname = local_name(c_name.as_ref());
                        if cname == b"e" {
                            base =
                                Some(parse_element(reader, child, depth + 1, node_count, budget)?);
                        } else if cname == b"sub" {
                            sub =
                                Some(parse_element(reader, child, depth + 1, node_count, budget)?);
                        } else {
                            skip_element(reader, child)?;
                        }
                    }
                    Ok(Event::End(ref e)) if e.name().as_ref() == end_name => break,
                    Ok(Event::Eof) => break,
                    Err(e) => return Err(OmmlError::Xml(e.to_string())),
                    _ => {}
                }
            }
            Ok(MathElement::Subscript {
                base: Box::new(base.unwrap_or(MathElement::Text(String::new()))),
                sub: Box::new(sub.unwrap_or(MathElement::Text(String::new()))),
            })
        }
        b"sSubSup" => {
            let mut base = None;
            let mut sub = None;
            let mut sup = None;
            let end_name = start.name().as_ref().to_vec();
            loop {
                match reader.read_event() {
                    Ok(Event::Start(ref child)) => {
                        let c_name = child.name();
                        let cname = local_name(c_name.as_ref());
                        if cname == b"e" {
                            base =
                                Some(parse_element(reader, child, depth + 1, node_count, budget)?);
                        } else if cname == b"sub" {
                            sub =
                                Some(parse_element(reader, child, depth + 1, node_count, budget)?);
                        } else if cname == b"sup" {
                            sup =
                                Some(parse_element(reader, child, depth + 1, node_count, budget)?);
                        } else {
                            skip_element(reader, child)?;
                        }
                    }
                    Ok(Event::End(ref e)) if e.name().as_ref() == end_name => break,
                    Ok(Event::Eof) => break,
                    Err(e) => return Err(OmmlError::Xml(e.to_string())),
                    _ => {}
                }
            }
            Ok(MathElement::SubSup {
                base: Box::new(base.unwrap_or(MathElement::Text(String::new()))),
                sub: Box::new(sub.unwrap_or(MathElement::Text(String::new()))),
                sup: Box::new(sup.unwrap_or(MathElement::Text(String::new()))),
            })
        }
        b"rad" => {
            let mut base = None;
            let mut deg = None;
            let end_name = start.name().as_ref().to_vec();
            loop {
                match reader.read_event() {
                    Ok(Event::Start(ref child)) => {
                        let c_name = child.name();
                        let cname = local_name(c_name.as_ref());
                        if cname == b"e" {
                            base =
                                Some(parse_element(reader, child, depth + 1, node_count, budget)?);
                        } else if cname == b"deg" {
                            deg =
                                Some(parse_element(reader, child, depth + 1, node_count, budget)?);
                        } else {
                            skip_element(reader, child)?;
                        }
                    }
                    Ok(Event::End(ref e)) if e.name().as_ref() == end_name => break,
                    Ok(Event::Eof) => break,
                    Err(e) => return Err(OmmlError::Xml(e.to_string())),
                    _ => {}
                }
            }
            Ok(MathElement::Radical {
                base: Box::new(base.unwrap_or(MathElement::Text(String::new()))),
                deg: deg.map(Box::new),
            })
        }
        b"d" => {
            let mut open = '(';
            let mut close = ')';
            let mut content = Vec::new();
            let end_name = start.name().as_ref().to_vec();
            loop {
                match reader.read_event() {
                    Ok(Event::Start(ref child)) => {
                        let c_name = child.name();
                        let cname = local_name(c_name.as_ref());
                        if cname == b"e" {
                            let elem = parse_element(reader, child, depth + 1, node_count, budget)?;
                            content.push(elem);
                        } else if cname == b"dPr" {
                            // parse delimiter properties (begChr, endChr)
                            parse_dpr(reader, child, &mut open, &mut close)?;
                        } else {
                            skip_element(reader, child)?;
                        }
                    }
                    Ok(Event::End(ref e)) if e.name().as_ref() == end_name => break,
                    Ok(Event::Eof) => break,
                    Err(e) => return Err(OmmlError::Xml(e.to_string())),
                    _ => {}
                }
            }
            Ok(MathElement::Delimiter {
                open,
                close,
                content,
            })
        }
        _ => {
            skip_element(reader, start)?;
            Ok(MathElement::Text(String::new()))
        }
    }
}

fn parse_dpr(
    reader: &mut Reader<&[u8]>,
    start: &BytesStart<'_>,
    open: &mut char,
    close: &mut char,
) -> Result<(), OmmlError> {
    let end_name = start.name().as_ref().to_vec();
    loop {
        match reader.read_event() {
            Ok(Event::Empty(ref child)) => {
                let c_name = child.name();
                let cname = local_name(c_name.as_ref());
                for attr in child.attributes().flatten() {
                    if local_name(attr.key.as_ref()) == b"val" {
                        let val = String::from_utf8_lossy(&attr.value);
                        if let Some(ch) = val.chars().next() {
                            if cname == b"begChr" {
                                *open = ch;
                            } else if cname == b"endChr" {
                                *close = ch;
                            }
                        }
                    }
                }
            }
            Ok(Event::End(ref e)) if e.name().as_ref() == end_name => break,
            Ok(Event::Eof) => break,
            Err(e) => return Err(OmmlError::Xml(e.to_string())),
            _ => {}
        }
    }
    Ok(())
}

fn skip_element(reader: &mut Reader<&[u8]>, start: &BytesStart<'_>) -> Result<(), OmmlError> {
    let end_name = start.name().as_ref().to_vec();
    let mut depth = 1;
    loop {
        match reader.read_event() {
            Ok(Event::Start(_)) => depth += 1,
            Ok(Event::End(ref e)) => {
                if depth == 1 && e.name().as_ref() == end_name {
                    break;
                }
                depth -= 1;
                if depth == 0 {
                    break;
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(OmmlError::Xml(e.to_string())),
            _ => {}
        }
    }
    Ok(())
}

fn local_name(qname: &[u8]) -> &[u8] {
    if let Some(idx) = qname.iter().position(|&b| b == b':') {
        &qname[idx + 1..]
    } else {
        qname
    }
}
