//! Structural read of a PDF file (ISO 32000-1 §7.5 cross-reference, §7.7
//! document structure, §7.8 page tree, §9.6 fonts, §8.9 images, §12.3
//! outlines, §12.5.6.5 link annotations, §12.6.4.7 URI actions, §14.3.3
//! document information).
//!
//! This parser does not share objects with the writer. It accepts a classic
//! cross-reference table or one cross-reference stream, including objects
//! stored in an object stream, and a single FlateDecode filter.

use std::collections::{BTreeMap, BTreeSet};
use std::io::Read;

const MAX_OBJECTS: usize = 4096;
const MAX_INFLATE: u64 = 8 << 20;
const MAX_DEPTH: usize = 32;

#[derive(Clone, Debug, PartialEq)]
pub struct PdfReport {
    pub pages: Vec<(f32, f32)>,
    pub producer: Option<String>,
    pub title: Option<String>,
    pub outlines: Vec<String>,
    pub fonts: usize,
    pub fonts_with_to_unicode: usize,
    pub image_xobjects: usize,
    pub link_uris: Vec<String>,
    pub has_creation_date: bool,
    pub has_mod_date: bool,
}

/// Destination read from PDF bytes, with raw PDF bottom-left coordinates.
/// `page` is resolved by identity in the file's page tree, not by object number.
#[derive(Clone, Debug, PartialEq)]
pub struct PdfDestinationReport {
    pub page: usize,
    pub page_object: u32,
    pub x: f32,
    pub y: f32,
    pub zoom: Option<f32>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PdfGoToReport {
    pub source_page: usize,
    pub rect: [f32; 4],
    pub destination: PdfDestinationReport,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PdfNavigationReport {
    pub links: Vec<PdfGoToReport>,
    pub bookmarks: BTreeMap<String, PdfDestinationReport>,
}

/// Independently follow xref/object streams, page tree and /GoTo /D references.
/// This is deliberately not a search for writer-produced strings or objects.
pub fn inspect_navigation(bytes: &[u8]) -> Result<PdfNavigationReport, &'static str> {
    if !bytes.starts_with(b"%PDF-") {
        return Err("pdf header");
    }
    let mut file = read_xref(bytes, startxref(bytes)?, 0)?;
    let catalog = file.object(file.root)?;
    let catalog = dict_of(&catalog).ok_or("catalog")?;
    let pages_ref = catalog.get("Pages").and_then(as_ref).ok_or("pages")?;
    let mut stats = PageStats::default();
    walk_pages(&mut file, pages_ref, None, &BTreeMap::new(), &mut stats, 0)?;
    let mut links = Vec::new();
    for (source_page, rect, value) in &stats.goto_links {
        links.push(PdfGoToReport {
            source_page: *source_page,
            rect: *rect,
            destination: read_xyz(&mut file, value, &stats)?,
        });
    }
    let mut bookmarks = BTreeMap::new();
    if let Some(names) = catalog.get("Names") {
        let names = resolve_dict(&mut file, names)?;
        if let Some(dests) = names.get("Dests") {
            read_named_destinations(
                &mut file,
                dests,
                &stats,
                &mut bookmarks,
                &mut BTreeSet::new(),
                0,
            )?;
        }
    }
    Ok(PdfNavigationReport { links, bookmarks })
}

fn read_xyz(
    file: &mut File<'_>,
    value: &Value,
    stats: &PageStats,
) -> Result<PdfDestinationReport, &'static str> {
    let mut value = resolve_value(file, value)?;
    if let Value::Dict(dict) = &value {
        value = resolve_value(file, dict.get("D").ok_or("destination dictionary")?)?;
    }
    let array = as_array(&value).ok_or("destination array")?;
    if array.len() != 5 || as_name(&array[1]) != Some("XYZ") {
        return Err("xyz destination");
    }
    let page_object = as_ref(&array[0]).ok_or("destination page reference")?;
    let page = stats
        .page_objects
        .iter()
        .position(|object| *object == page_object)
        .ok_or("destination outside page tree")?;
    let x = as_number(&array[2]).ok_or("destination x")?;
    let y = as_number(&array[3]).ok_or("destination y")?;
    let zoom = match &array[4] {
        Value::Null => None,
        value => Some(
            as_number(value)
                .filter(|n| n.is_finite() && *n >= 0.0)
                .ok_or("destination zoom")?,
        ),
    };
    let (width, height) = stats.pages[page];
    if !x.is_finite() || !y.is_finite() || x < 0.0 || y < 0.0 || x > width || y > height {
        return Err("destination coordinates");
    }
    Ok(PdfDestinationReport {
        page,
        page_object,
        x,
        y,
        zoom,
    })
}

fn read_named_destinations(
    file: &mut File<'_>,
    value: &Value,
    stats: &PageStats,
    out: &mut BTreeMap<String, PdfDestinationReport>,
    seen: &mut BTreeSet<u32>,
    depth: usize,
) -> Result<(), &'static str> {
    if depth > MAX_DEPTH || seen.len() >= MAX_OBJECTS {
        return Err("destination tree depth");
    }
    if let Value::Ref(number) = value
        && !seen.insert(*number)
    {
        return Err("destination tree cycle");
    }
    let dict = resolve_dict(file, value)?;
    if let Some(kids) = dict.get("Kids").and_then(as_array) {
        for child in kids {
            read_named_destinations(file, child, stats, out, seen, depth + 1)?;
        }
    }
    if let Some(names) = dict.get("Names").and_then(as_array) {
        if names.len() % 2 != 0 {
            return Err("destination name pairs");
        }
        for pair in names.as_chunks::<2>().0 {
            let Value::Str(name) = &pair[0] else {
                return Err("destination name");
            };
            if out.len() >= MAX_OBJECTS
                || out
                    .insert(name.clone(), read_xyz(file, &pair[1], stats)?)
                    .is_some()
            {
                return Err("destination name count or duplicate");
            }
        }
    }
    Ok(())
}

fn goto_link(
    file: &mut File<'_>,
    value: &Value,
) -> Result<Option<([f32; 4], Value)>, &'static str> {
    let dict = resolve_dict(file, value)?;
    if name(&dict, "Subtype") != Some("Link") {
        return Ok(None);
    }
    let Some(action) = dict.get("A") else {
        return Ok(None);
    };
    let action = resolve_dict(file, action)?;
    if name(&action, "S") != Some("GoTo") {
        return Ok(None);
    }
    let destination = action.get("D").ok_or("goto destination")?.clone();
    let rect = dict.get("Rect").and_then(as_array).ok_or("goto rect")?;
    if rect.len() != 4 {
        return Err("goto rect");
    }
    let mut coords = [0.0; 4];
    for (coord, value) in coords.iter_mut().zip(rect) {
        *coord = as_number(value)
            .filter(|n| n.is_finite())
            .ok_or("goto rect coordinate")?;
    }
    Ok(Some((coords, destination)))
}

#[derive(Clone, Debug)]
enum Value {
    Null,
    Bool,
    Int(i64),
    Real(f32),
    Name(String),
    Str(String),
    Ref(u32),
    Array(Vec<Value>),
    Dict(BTreeMap<String, Value>),
    Stream {
        dict: BTreeMap<String, Value>,
        data: Vec<u8>,
    },
}

#[derive(Clone, Copy)]
enum Loc {
    Offset(usize),
    InStream { container: u32, index: u32 },
}

struct File<'a> {
    data: &'a [u8],
    xref: BTreeMap<u32, Loc>,
    root: u32,
    info: Option<u32>,
}

pub fn inspect(bytes: &[u8]) -> Result<PdfReport, &'static str> {
    if bytes.len() < 16 || !bytes.starts_with(b"%PDF-") {
        return Err("pdf header");
    }
    let start = startxref(bytes)?;
    let mut file = read_xref(bytes, start, 0)?;
    let catalog = file.object(file.root)?;
    let catalog = dict_of(&catalog).ok_or("catalog")?;
    if name(catalog, "Type").is_some_and(|kind| kind != "Catalog") && catalog.contains_key("Type") {
        return Err("catalog type");
    }
    let pages_ref = catalog.get("Pages").and_then(as_ref).ok_or("pages")?;
    let mut stats = PageStats::default();
    walk_pages(&mut file, pages_ref, None, &BTreeMap::new(), &mut stats, 0)?;
    if stats.pages.is_empty() {
        return Err("no pages");
    }
    let info = match file.info {
        Some(info) => info_fields(&file.object(info)?)?,
        None => InfoFields::default(),
    };
    let outlines = match catalog.get("Outlines").and_then(as_ref) {
        Some(root) => outline_titles(&mut file, root)?,
        None => Vec::new(),
    };
    Ok(PdfReport {
        pages: stats.pages,
        producer: info.producer,
        title: info.title,
        outlines,
        fonts: stats.fonts,
        fonts_with_to_unicode: stats.fonts_with_to_unicode,
        image_xobjects: stats.images,
        link_uris: stats.link_uris,
        has_creation_date: info.has_creation_date,
        has_mod_date: info.has_mod_date,
    })
}

#[derive(Default)]
struct PageStats {
    pages: Vec<(f32, f32)>,
    page_objects: Vec<u32>,
    goto_links: Vec<(usize, [f32; 4], Value)>,
    link_uris: Vec<String>,
    fonts: usize,
    fonts_with_to_unicode: usize,
    images: usize,
    seen: BTreeSet<u32>,
}

#[derive(Default)]
struct InfoFields {
    producer: Option<String>,
    title: Option<String>,
    has_creation_date: bool,
    has_mod_date: bool,
}

fn startxref(bytes: &[u8]) -> Result<usize, &'static str> {
    let marker = b"startxref";
    let mut found = None;
    let mut index = 0;
    while index + marker.len() <= bytes.len() {
        if &bytes[index..index + marker.len()] == marker {
            found = Some(index);
        }
        index += 1;
    }
    let at = found.ok_or("startxref")?;
    let mut cursor = Cursor {
        data: bytes,
        index: at + marker.len(),
    };
    cursor.skip();
    let offset = cursor.int().ok_or("startxref offset")?;
    let offset = usize::try_from(offset).map_err(|_| "startxref offset")?;
    if offset >= bytes.len() {
        return Err("startxref offset");
    }
    let tail = &bytes[at..];
    if !tail.windows(5).any(|window| window == b"%%EOF") {
        return Err("eof");
    }
    Ok(offset)
}

fn read_xref(data: &[u8], offset: usize, depth: usize) -> Result<File<'_>, &'static str> {
    if depth > 4 {
        return Err("xref chain");
    }
    let mut cursor = Cursor {
        data,
        index: offset,
    };
    cursor.skip();
    if cursor.eat(b"xref") {
        let mut xref = BTreeMap::new();
        cursor.skip();
        while cursor.peek().is_some_and(|byte| byte.is_ascii_digit()) {
            let start = cursor.int().ok_or("xref start")? as u32;
            cursor.skip();
            let count = cursor.int().ok_or("xref count")? as usize;
            if count > MAX_OBJECTS {
                return Err("xref count");
            }
            for index in 0..count {
                cursor.skip();
                let entry_offset = cursor.int().ok_or("xref entry")?;
                cursor.skip();
                let _generation = cursor.int().ok_or("xref generation")?;
                cursor.skip();
                let flag = cursor.bump().ok_or("xref flag")?;
                if flag == b'n' {
                    let at = usize::try_from(entry_offset).map_err(|_| "xref entry")?;
                    xref.insert(start + index as u32, Loc::Offset(at));
                }
            }
            cursor.skip();
        }
        cursor.eat(b"trailer");
        cursor.skip();
        let trailer = match cursor.value()? {
            Value::Dict(dict) => dict,
            _ => return Err("trailer"),
        };
        finish_trailer(data, xref, &trailer, depth)
    } else {
        let object = parse_object_at(data, offset)?;
        let (dict, stream) = match object {
            Value::Stream { dict, data } => (dict, data),
            _ => return Err("xref stream"),
        };
        if name(&dict, "Type").is_some_and(|kind| kind != "XRef") && dict.contains_key("Type") {
            return Err("xref type");
        }
        let size = dict.get("Size").and_then(as_int).ok_or("xref size")? as u32;
        let widths = int_array(dict.get("W")).ok_or("xref widths")?;
        if widths.len() != 3 {
            return Err("xref widths");
        }
        let index = match dict.get("Index") {
            Some(value) => int_array(Some(value)).ok_or("xref index")?,
            None => vec![0, i64::from(size)],
        };
        if index.len() % 2 != 0 {
            return Err("xref index");
        }
        let mut xref = BTreeMap::new();
        let mut stream_at = 0usize;
        for pair in index.chunks(2) {
            let start = pair[0] as u32;
            let count = pair[1] as usize;
            for entry in 0..count {
                let (kind, field, third) = read_xref_fields(&stream, &mut stream_at, &widths)?;
                let number = start + entry as u32;
                match kind {
                    0 => {}
                    1 => {
                        xref.insert(number, Loc::Offset(field as usize));
                    }
                    2 => {
                        xref.insert(
                            number,
                            Loc::InStream {
                                container: field as u32,
                                index: third as u32,
                            },
                        );
                    }
                    _ => return Err("xref type"),
                }
            }
        }
        finish_trailer(data, xref, &dict, depth)
    }
}

fn read_xref_fields(
    stream: &[u8],
    at: &mut usize,
    widths: &[i64],
) -> Result<(u64, u64, u64), &'static str> {
    let kind = if widths[0] == 0 {
        1
    } else {
        read_be(stream, at, widths[0] as usize)?
    };
    let field = read_be(stream, at, widths[1] as usize)?;
    let third = read_be(stream, at, widths[2] as usize)?;
    Ok((kind, field, third))
}

fn read_be(stream: &[u8], at: &mut usize, width: usize) -> Result<u64, &'static str> {
    if width > 8 || *at + width > stream.len() {
        return Err("xref field");
    }
    let mut value = 0u64;
    for _ in 0..width {
        value = (value << 8) | u64::from(stream[*at]);
        *at += 1;
    }
    Ok(value)
}

fn finish_trailer<'a>(
    data: &'a [u8],
    mut xref: BTreeMap<u32, Loc>,
    trailer: &BTreeMap<String, Value>,
    depth: usize,
) -> Result<File<'a>, &'static str> {
    if let Some(prev) = trailer.get("Prev").and_then(as_int) {
        let older = read_xref(data, prev as usize, depth + 1)?;
        for (number, loc) in older.xref {
            xref.entry(number).or_insert(loc);
        }
    }
    let root = trailer.get("Root").and_then(as_ref).ok_or("root")?;
    let info = trailer.get("Info").and_then(as_ref);
    Ok(File {
        data,
        xref,
        root,
        info,
    })
}

impl File<'_> {
    fn object(&mut self, number: u32) -> Result<Value, &'static str> {
        let loc = *self.xref.get(&number).ok_or("missing object")?;
        match loc {
            Loc::Offset(offset) => parse_indirect(self.data, offset),
            Loc::InStream { container, index } => {
                let container_object = match self.xref.get(&container).copied() {
                    Some(Loc::Offset(offset)) => parse_indirect(self.data, offset)?,
                    _ => return Err("object stream"),
                };
                let Value::Stream { dict, data } = container_object else {
                    return Err("object stream");
                };
                let count = dict.get("N").and_then(as_int).ok_or("object stream")? as usize;
                let first = dict.get("First").and_then(as_int).ok_or("object stream")? as usize;
                if count > MAX_OBJECTS || first > data.len() {
                    return Err("object stream");
                }
                let header = std::str::from_utf8(&data[..first]).map_err(|_| "object stream")?;
                let pairs: Vec<(u32, usize)> = header
                    .split_whitespace()
                    .collect::<Vec<_>>()
                    .chunks(2)
                    .filter_map(|pair| {
                        let number = pair.first()?.parse().ok()?;
                        let offset = pair.get(1)?.parse().ok()?;
                        Some((number, offset))
                    })
                    .collect();
                if pairs.len() != count || index as usize >= pairs.len() {
                    return Err("object stream");
                }
                let at = first + pairs[index as usize].1;
                if at > data.len() {
                    return Err("object stream");
                }
                let mut cursor = Cursor {
                    data: &data,
                    index: at,
                };
                cursor.value()
            }
        }
    }
}

fn parse_indirect(data: &[u8], offset: usize) -> Result<Value, &'static str> {
    if offset >= data.len() {
        return Err("object offset");
    }
    let mut cursor = Cursor {
        data,
        index: offset,
    };
    cursor.skip();
    let _number = cursor.int().ok_or("object number")?;
    cursor.skip();
    let _generation = cursor.int().ok_or("object generation")?;
    cursor.skip();
    if !cursor.eat(b"obj") {
        return Err("object");
    }
    cursor.skip();
    let value = cursor.value()?;
    if let Value::Dict(dict) = &value {
        cursor.skip();
        if cursor.eat(b"stream") {
            if cursor.bump() == Some(b'\r') {
                cursor.bump();
            }
            let length = dict.get("Length").and_then(as_int).ok_or("stream length")?;
            if length < 0 {
                return Err("stream length");
            }
            let length = length as usize;
            if cursor.index + length > data.len() {
                return Err("stream length");
            }
            let raw = data[cursor.index..cursor.index + length].to_vec();
            let decoded = match decode_stream(dict, &raw) {
                Ok(decoded) => decoded,
                Err(_)
                    if matches!(name(dict, "Type"), Some("XRef" | "ObjStm"))
                        || dict.contains_key("W") =>
                {
                    return Err("flate");
                }
                // Page content and images only need the dictionary for this check.
                Err(_) => raw,
            };
            return Ok(Value::Stream {
                dict: dict.clone(),
                data: decoded,
            });
        }
    }
    Ok(value)
}

fn parse_object_at(data: &[u8], offset: usize) -> Result<Value, &'static str> {
    parse_indirect(data, offset)
}

fn decode_stream(dict: &BTreeMap<String, Value>, raw: &[u8]) -> Result<Vec<u8>, &'static str> {
    let filters = match dict.get("Filter") {
        None => return Ok(raw.to_vec()),
        Some(Value::Name(name)) => vec![name.as_str()],
        Some(Value::Array(items)) => items.iter().filter_map(as_name).collect(),
        Some(_) => return Err("stream filter"),
    };
    if filters.is_empty() {
        return Ok(raw.to_vec());
    }
    if filters != ["FlateDecode"] {
        return Err("stream filter");
    }
    let decoder = flate2::read::ZlibDecoder::new(raw);
    let mut out = Vec::new();
    decoder
        .take(MAX_INFLATE)
        .read_to_end(&mut out)
        .map_err(|_| "flate")?;
    Ok(out)
}

fn walk_pages(
    file: &mut File<'_>,
    number: u32,
    inherited_box: Option<(f32, f32)>,
    inherited_resources: &BTreeMap<String, Value>,
    stats: &mut PageStats,
    depth: usize,
) -> Result<(), &'static str> {
    if !stats.seen.insert(number) || depth > MAX_DEPTH || stats.pages.len() > MAX_OBJECTS {
        return Err("page tree");
    }
    let object = file.object(number)?;
    let dict = dict_of(&object).ok_or("page object")?.clone();
    let resources = merge_resources(inherited_resources, dict.get("Resources"), file)?;
    let kind = name(&dict, "Type");
    if kind == Some("Pages") || (kind.is_none() && dict.contains_key("Kids")) {
        let kids = dict.get("Kids").and_then(as_array).ok_or("kids")?;
        let media = media_box(&dict).or(inherited_box);
        let child_refs: Vec<u32> = kids.iter().filter_map(as_ref).collect();
        for child in child_refs {
            walk_pages(file, child, media, &resources, stats, depth + 1)?;
        }
        return Ok(());
    }
    let media = media_box(&dict).or(inherited_box).ok_or("media box")?;
    if media.0 <= 0.0 || media.1 <= 0.0 {
        return Err("media box");
    }
    stats.pages.push(media);
    stats.page_objects.push(number);
    count_resources(
        file,
        &resources,
        &mut stats.fonts,
        &mut stats.fonts_with_to_unicode,
        &mut stats.images,
        0,
    )?;
    if let Some(annots) = dict.get("Annots").and_then(as_array) {
        for annot in annots {
            if let Some(uri) = link_uri(file, annot)? {
                stats.link_uris.push(uri);
            }
            if let Some((rect, destination)) = goto_link(file, annot)? {
                if stats.goto_links.len() >= MAX_OBJECTS {
                    return Err("goto count");
                }
                stats
                    .goto_links
                    .push((stats.pages.len() - 1, rect, destination));
            }
        }
    }
    Ok(())
}

fn link_uri(file: &mut File<'_>, value: &Value) -> Result<Option<String>, &'static str> {
    let dict = match value {
        Value::Ref(number) => dict_of(&file.object(*number)?).ok_or("annotation")?.clone(),
        Value::Dict(dict) => dict.clone(),
        _ => return Ok(None),
    };
    if name(&dict, "Subtype") != Some("Link") {
        return Ok(None);
    }
    let Some(action) = dict.get("A") else {
        return Ok(None);
    };
    let action = resolve_dict(file, action)?;
    if name(&action, "S") != Some("URI") {
        return Ok(None);
    }
    Ok(match action.get("URI") {
        Some(Value::Str(text)) => Some(text.clone()),
        Some(Value::Ref(number)) => match file.object(*number)? {
            Value::Str(text) => Some(text),
            _ => None,
        },
        _ => None,
    })
}

fn merge_resources(
    inherited: &BTreeMap<String, Value>,
    own: Option<&Value>,
    file: &mut File<'_>,
) -> Result<BTreeMap<String, Value>, &'static str> {
    let mut merged = inherited.clone();
    let Some(own) = own else {
        return Ok(merged);
    };
    let own = resolve_dict(file, own)?;
    for (key, value) in own {
        merged.insert(key, value);
    }
    Ok(merged)
}

fn count_resources(
    file: &mut File<'_>,
    resources: &BTreeMap<String, Value>,
    fonts: &mut usize,
    fonts_with_to_unicode: &mut usize,
    images: &mut usize,
    depth: usize,
) -> Result<(), &'static str> {
    if depth > 4 {
        return Ok(());
    }
    if let Some(font_dict) = resources.get("Font") {
        let font_dict = resolve_dict(file, font_dict)?;
        for (_name, font) in font_dict {
            let font = resolve_dict(file, &font)?;
            *fonts += 1;
            if font.contains_key("ToUnicode") {
                *fonts_with_to_unicode += 1;
            }
        }
    }
    if let Some(xobjects) = resources.get("XObject") {
        let xobjects = resolve_dict(file, xobjects)?;
        for (_name, object) in xobjects {
            let resolved = resolve_value(file, &object)?;
            let Some(dict) = dict_of(&resolved) else {
                continue;
            };
            match name(dict, "Subtype") {
                Some("Image") => *images += 1,
                Some("Form") => {
                    if let Some(inner) = dict.get("Resources") {
                        let inner = resolve_dict(file, inner)?;
                        count_resources(
                            file,
                            &inner,
                            fonts,
                            fonts_with_to_unicode,
                            images,
                            depth + 1,
                        )?;
                    }
                }
                _ => {}
            }
        }
    }
    Ok(())
}

fn outline_titles(file: &mut File<'_>, root: u32) -> Result<Vec<String>, &'static str> {
    let object = file.object(root)?;
    let dict = dict_of(&object).ok_or("outline")?.clone();
    let mut titles = Vec::new();
    let mut seen = BTreeSet::new();
    if let Some(first) = dict.get("First").and_then(as_ref) {
        walk_outline(file, first, &mut titles, &mut seen, 0)?;
    }
    Ok(titles)
}

fn walk_outline(
    file: &mut File<'_>,
    number: u32,
    titles: &mut Vec<String>,
    seen: &mut BTreeSet<u32>,
    depth: usize,
) -> Result<(), &'static str> {
    if !seen.insert(number) || depth > MAX_DEPTH || titles.len() > MAX_OBJECTS {
        return Err("outline");
    }
    let object = file.object(number)?;
    let dict = dict_of(&object).ok_or("outline item")?.clone();
    if let Some(Value::Str(title)) = dict.get("Title") {
        titles.push(title.clone());
    }
    if let Some(child) = dict.get("First").and_then(as_ref) {
        walk_outline(file, child, titles, seen, depth + 1)?;
    }
    if let Some(next) = dict.get("Next").and_then(as_ref) {
        walk_outline(file, next, titles, seen, depth)?;
    }
    Ok(())
}

fn info_fields(value: &Value) -> Result<InfoFields, &'static str> {
    let dict = dict_of(value).ok_or("info")?;
    Ok(InfoFields {
        producer: match dict.get("Producer") {
            Some(Value::Str(text)) => Some(text.clone()),
            _ => None,
        },
        title: match dict.get("Title") {
            Some(Value::Str(text)) => Some(text.clone()),
            _ => None,
        },
        has_creation_date: dict.contains_key("CreationDate"),
        has_mod_date: dict.contains_key("ModDate"),
    })
}

fn media_box(dict: &BTreeMap<String, Value>) -> Option<(f32, f32)> {
    let array = as_array(dict.get("MediaBox")?)?;
    if array.len() != 4 {
        return None;
    }
    let left = as_number(&array[0])?;
    let bottom = as_number(&array[1])?;
    let right = as_number(&array[2])?;
    let top = as_number(&array[3])?;
    Some((right - left, top - bottom))
}

fn resolve_dict(
    file: &mut File<'_>,
    value: &Value,
) -> Result<BTreeMap<String, Value>, &'static str> {
    match resolve_value(file, value)? {
        Value::Dict(dict) => Ok(dict),
        Value::Stream { dict, .. } => Ok(dict),
        _ => Err("dictionary"),
    }
}

fn resolve_value(file: &mut File<'_>, value: &Value) -> Result<Value, &'static str> {
    match value {
        Value::Ref(number) => file.object(*number),
        other => Ok(other.clone()),
    }
}

fn dict_of(value: &Value) -> Option<&BTreeMap<String, Value>> {
    match value {
        Value::Dict(dict) | Value::Stream { dict, .. } => Some(dict),
        _ => None,
    }
}

fn name<'a>(dict: &'a BTreeMap<String, Value>, key: &str) -> Option<&'a str> {
    match dict.get(key) {
        Some(Value::Name(name)) => Some(name),
        _ => None,
    }
}

fn as_ref(value: &Value) -> Option<u32> {
    match value {
        Value::Ref(number) => Some(*number),
        _ => None,
    }
}

fn as_int(value: &Value) -> Option<i64> {
    match value {
        Value::Int(number) => Some(*number),
        Value::Real(number)
            if number.is_finite() && number.fract() == 0.0 && number.abs() <= i64::MAX as f32 =>
        {
            Some(*number as i64)
        }
        _ => None,
    }
}

fn as_number(value: &Value) -> Option<f32> {
    match value {
        Value::Int(number) => Some(*number as f32),
        Value::Real(number) => Some(*number),
        _ => None,
    }
}

fn as_array(value: &Value) -> Option<&[Value]> {
    match value {
        Value::Array(items) => Some(items),
        _ => None,
    }
}

fn as_name(value: &Value) -> Option<&str> {
    match value {
        Value::Name(name) => Some(name),
        _ => None,
    }
}

fn int_array(value: Option<&Value>) -> Option<Vec<i64>> {
    let array = as_array(value?)?;
    array.iter().map(as_int).collect()
}

struct Cursor<'a> {
    data: &'a [u8],
    index: usize,
}

impl<'a> Cursor<'a> {
    fn peek(&self) -> Option<u8> {
        self.data.get(self.index).copied()
    }

    fn bump(&mut self) -> Option<u8> {
        let byte = self.peek()?;
        self.index += 1;
        Some(byte)
    }

    fn eat(&mut self, token: &[u8]) -> bool {
        if self
            .data
            .get(self.index..)
            .is_some_and(|rest| rest.starts_with(token))
        {
            self.index += token.len();
            true
        } else {
            false
        }
    }

    fn skip(&mut self) {
        loop {
            while self.peek().is_some_and(is_ws) {
                self.index += 1;
            }
            if self.peek() == Some(b'%') {
                while self
                    .peek()
                    .is_some_and(|byte| byte != b'\n' && byte != b'\r')
                {
                    self.index += 1;
                }
                continue;
            }
            break;
        }
    }

    fn int(&mut self) -> Option<i64> {
        self.skip();
        let start = self.index;
        if matches!(self.peek(), Some(b'+' | b'-')) {
            self.index += 1;
        }
        let digits = self.index;
        while self.peek().is_some_and(|byte| byte.is_ascii_digit()) {
            self.index += 1;
        }
        if self.index == digits {
            self.index = start;
            return None;
        }
        std::str::from_utf8(&self.data[start..self.index])
            .ok()?
            .parse()
            .ok()
    }

    fn value(&mut self) -> Result<Value, &'static str> {
        self.skip();
        match self.peek() {
            Some(b'<') if self.data.get(self.index + 1) == Some(&b'<') => self.dict(),
            Some(b'<') => self.hex_string(),
            Some(b'[') => self.array(),
            Some(b'(') => self.literal(),
            Some(b'/') => Ok(Value::Name(self.name()?)),
            Some(b't') if self.eat(b"true") => Ok(Value::Bool),
            Some(b'f') if self.eat(b"false") => Ok(Value::Bool),
            Some(b'n') if self.eat(b"null") => Ok(Value::Null),
            Some(b'+' | b'-' | b'.' | b'0'..=b'9') => self.number_or_ref(),
            _ => Err("pdf value"),
        }
    }

    fn dict(&mut self) -> Result<Value, &'static str> {
        if !self.eat(b"<<") {
            return Err("dictionary");
        }
        let mut dict = BTreeMap::new();
        loop {
            self.skip();
            if self.eat(b">>") {
                return Ok(Value::Dict(dict));
            }
            if self.peek() != Some(b'/') {
                return Err("dictionary");
            }
            let key = self.name()?;
            let value = self.value()?;
            dict.insert(key, value);
        }
    }

    fn array(&mut self) -> Result<Value, &'static str> {
        if !self.eat(b"[") {
            return Err("array");
        }
        let mut items = Vec::new();
        loop {
            self.skip();
            if self.eat(b"]") {
                return Ok(Value::Array(items));
            }
            items.push(self.value()?);
            if items.len() > MAX_OBJECTS {
                return Err("array");
            }
        }
    }

    fn number_or_ref(&mut self) -> Result<Value, &'static str> {
        let number = self.number().ok_or("number")?;
        let after = self.index;
        self.skip();
        if let Value::Int(integer) = number
            && self.peek().is_some_and(|byte| byte.is_ascii_digit())
        {
            let generation_at = self.index;
            if let Some(generation) = self.int() {
                self.skip();
                if generation >= 0 && self.eat(b"R") && (0..=u32::MAX as i64).contains(&integer) {
                    return Ok(Value::Ref(integer as u32));
                }
            }
            self.index = generation_at;
        }
        self.index = after;
        Ok(number)
    }

    fn number(&mut self) -> Option<Value> {
        let start = self.index;
        if matches!(self.peek(), Some(b'+' | b'-')) {
            self.index += 1;
        }
        let mut saw_digit = false;
        let mut saw_dot = false;
        while self.peek().is_some_and(|byte| byte.is_ascii_digit()) {
            saw_digit = true;
            self.index += 1;
        }
        if self.peek() == Some(b'.') {
            saw_dot = true;
            self.index += 1;
            while self.peek().is_some_and(|byte| byte.is_ascii_digit()) {
                saw_digit = true;
                self.index += 1;
            }
        }
        if !saw_digit {
            self.index = start;
            return None;
        }
        let text = std::str::from_utf8(&self.data[start..self.index]).ok()?;
        if saw_dot {
            text.parse().ok().map(Value::Real)
        } else {
            text.parse().ok().map(Value::Int)
        }
    }

    fn name(&mut self) -> Result<String, &'static str> {
        if !self.eat(b"/") {
            return Err("name");
        }
        let mut out = String::new();
        while let Some(byte) = self.peek() {
            if is_delimiter(byte) || is_ws(byte) {
                break;
            }
            self.index += 1;
            if byte == b'#' {
                let hex = self.data.get(self.index..self.index + 2).ok_or("name")?;
                let value = u8::from_str_radix(std::str::from_utf8(hex).map_err(|_| "name")?, 16)
                    .map_err(|_| "name")?;
                self.index += 2;
                out.push(char::from(value));
            } else {
                out.push(char::from(byte));
            }
        }
        Ok(out)
    }

    fn literal(&mut self) -> Result<Value, &'static str> {
        if !self.eat(b"(") {
            return Err("string");
        }
        let mut bytes = Vec::new();
        let mut depth = 1i32;
        while depth > 0 {
            let byte = self.bump().ok_or("string")?;
            match byte {
                b'(' => {
                    depth += 1;
                    bytes.push(byte);
                }
                b')' => {
                    depth -= 1;
                    if depth > 0 {
                        bytes.push(byte);
                    }
                }
                b'\\' => {
                    let escaped = self.bump().ok_or("string")?;
                    let mapped = match escaped {
                        b'n' => b'\n',
                        b'r' => b'\r',
                        b't' => b'\t',
                        b'b' => 8,
                        b'f' => 12,
                        b'(' | b')' | b'\\' => escaped,
                        b'\n' => continue,
                        b'\r' => {
                            if self.peek() == Some(b'\n') {
                                self.index += 1;
                            }
                            continue;
                        }
                        b'0'..=b'7' => {
                            let mut value = u32::from(escaped - b'0');
                            for _ in 0..2 {
                                if let Some(digit) =
                                    self.peek().filter(|byte| (b'0'..=b'7').contains(byte))
                                {
                                    value = value * 8 + u32::from(digit - b'0');
                                    self.index += 1;
                                }
                            }
                            value as u8
                        }
                        other => other,
                    };
                    bytes.push(mapped);
                }
                other => bytes.push(other),
            }
        }
        Ok(Value::Str(decode_pdf_text(&bytes)))
    }

    fn hex_string(&mut self) -> Result<Value, &'static str> {
        if !self.eat(b"<") {
            return Err("string");
        }
        let mut nibbles = Vec::new();
        loop {
            self.skip();
            if self.eat(b">") {
                break;
            }
            let byte = self.bump().ok_or("string")?;
            let nibble = hex_value(byte).ok_or("string")?;
            nibbles.push(nibble);
        }
        if nibbles.len() % 2 == 1 {
            nibbles.push(0);
        }
        let bytes: Vec<u8> = nibbles
            .chunks(2)
            .map(|pair| (pair[0] << 4) | pair[1])
            .collect();
        Ok(Value::Str(decode_pdf_text(&bytes)))
    }
}

fn decode_pdf_text(bytes: &[u8]) -> String {
    if bytes.len() >= 2 && bytes[0] == 0xfe && bytes[1] == 0xff {
        let units: Vec<u16> = bytes[2..]
            .chunks(2)
            .map(|pair| u16::from_be_bytes([pair[0], *pair.get(1).unwrap_or(&0)]))
            .collect();
        return String::from_utf16_lossy(&units);
    }
    bytes.iter().map(|byte| char::from(*byte)).collect()
}

fn hex_value(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

fn is_ws(byte: u8) -> bool {
    matches!(byte, 0 | b'\t' | b'\n' | 0x0c | b'\r' | b' ')
}

fn is_delimiter(byte: u8) -> bool {
    matches!(
        byte,
        b'(' | b')' | b'<' | b'>' | b'[' | b']' | b'{' | b'}' | b'/' | b'%'
    )
}
