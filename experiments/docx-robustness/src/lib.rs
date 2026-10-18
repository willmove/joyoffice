//! Three budgeted targets. A panic must crash the process, never become OK.
use joyoffice_opc::{Budget, Package};
use joyword_docx::{ExportError, import};
use joyword_engine::{Command, Transaction};
use joyword_model::{Bias, Position, ScalarOffset};
use joyword_shell::docx::DocxSession;
use sha2::{Digest, Sha256};
pub const TARGET_SHA: &str = "a8d2c4e9f7a0f8dfed23e3be3843430bd1606970";
pub const MAX_INPUT: usize = 1 << 20;
pub fn budget() -> Budget {
    Budget {
        max_entries: 128,
        max_part_bytes: 1 << 20,
        max_total_bytes: 4 << 20,
        max_ratio: 100,
        ratio_floor_bytes: 4096,
        max_xml_depth: 32,
        max_xml_nodes: 20_000,
        max_relationships: 128,
    }
}
pub fn digest(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|n| format!("{n:02x}"))
        .collect()
}
pub fn json(text: &str) -> String {
    let mut out = String::from("\"");
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c < ' ' => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}
#[derive(Debug, PartialEq, Eq)]
pub enum ResultKind {
    Ok,
    Rejected,
    Invariant,
}
pub struct Report {
    pub kind: ResultKind,
    pub reason: String,
    pub checks: usize,
}
fn ok(checks: usize) -> Report {
    Report {
        kind: ResultKind::Ok,
        reason: String::new(),
        checks,
    }
}
fn reject(reason: impl ToString) -> Report {
    Report {
        kind: ResultKind::Rejected,
        reason: reason.to_string(),
        checks: 0,
    }
}
fn fail(reason: &str) -> Report {
    Report {
        kind: ResultKind::Invariant,
        reason: reason.into(),
        checks: 0,
    }
}
pub fn run(target: &str, bytes: &[u8], seed: u64) -> Report {
    if bytes.len() > MAX_INPUT {
        return reject("input byte ceiling");
    }
    match target {
        "opc" => match Package::read(bytes, &budget()) {
            Ok(p) => {
                if p.parts().map(|part| part.data.len() as u64).sum::<u64>()
                    > budget().max_total_bytes
                {
                    return fail("OPC inflated budget");
                }
                ok(1)
            }
            Err(e) => reject(e),
        },
        "import" => match (import(bytes, &budget()), import(bytes, &budget())) {
            (Ok(a), Ok(b)) => {
                if a.document.semantic_difference(&b.document).is_some()
                    || a.editability != b.editability
                {
                    return fail("import determinism");
                }
                if a.backing.original_bytes() != bytes {
                    return fail("import exact bytes");
                }
                ok(2)
            }
            (Err(a), Err(b)) if a == b => reject(a),
            _ => fail("nondeterministic import outcome"),
        },
        "roundtrip" => roundtrip(bytes, seed),
        _ => fail("unknown target"),
    }
}
fn roundtrip(bytes: &[u8], seed: u64) -> Report {
    let mut session = match DocxSession::open(bytes, &budget()) {
        Ok(s) => s,
        Err(e) => return reject(e),
    };
    let unchanged = match session.export_plan() {
        Ok(p) => p,
        Err(e) => return fail(&format!("unchanged plan: {e}")),
    };
    if unchanged.bytes() != bytes {
        return fail("unchanged package copy");
    }
    let Some(p) = session
        .session()
        .document()
        .body()
        .blocks
        .iter()
        .find_map(|b| b.as_paragraph())
    else {
        return reject("no paragraph for text target");
    };
    let before = session.session().document().snapshot();
    let initial_version = session.session().version();
    let backing = session.backing().original_bytes().to_vec();
    let start = Position {
        story: session.session().document().body_id(),
        paragraph: p.id(),
        offset: ScalarOffset(0),
        bias: Bias::After,
    };
    // Bounded generated insertions; engine remains the sole editor. Unicode,
    // XML escaping, whitespace and invalid controls form distinct seed cases.
    let text = match seed % 6 {
        0 => "甲👩‍💻e\u{301} <&> ",
        1 => "first",
        2 => "\t",
        3 => " x ",
        4 => "\u{1}",
        _ => "",
    };
    let tx = Transaction::new(
        initial_version,
        vec![Command::InsertText {
            at: start,
            text: text.into(),
        }],
    );
    if session.apply(tx).is_err() {
        if session.session().version() != initial_version
            || session
                .session()
                .document()
                .semantic_difference(&before)
                .is_some()
            || session.backing().original_bytes() != backing
        {
            return fail("rejected edit is not atomic");
        }
        return ok(4);
    }
    let original_text = before
        .body()
        .blocks
        .iter()
        .filter_map(|b| b.as_paragraph())
        .find(|p| p.id() == start.paragraph)
        .unwrap()
        .plain_text(|_| "\u{FFFC}".into());
    let mut reference = original_text.chars().collect::<Vec<_>>();
    reference.splice(0..0, text.chars());
    for step in 0..(seed % 4) {
        let command = if step % 2 == 0 {
            reference.splice(0..0, "丙e\u{301}".chars());
            Command::InsertText {
                at: start,
                text: "丙e\u{301}".into(),
            }
        } else {
            if reference.is_empty() {
                continue;
            }
            reference.remove(0);
            Command::DeleteRange {
                start,
                end: Position {
                    offset: ScalarOffset(1),
                    ..start
                },
            }
        };
        if session
            .apply(Transaction::new(session.session().version(), vec![command]))
            .is_err()
        {
            return fail("generated legal text sequence rejected");
        }
        let actual = session
            .session()
            .document()
            .body()
            .blocks
            .iter()
            .filter_map(|b| b.as_paragraph())
            .find(|p| p.id() == start.paragraph)
            .unwrap()
            .plain_text(|_| "\u{FFFC}".into());
        if actual != reference.iter().collect::<String>() {
            return fail("independent edit sequence text model");
        }
    }
    let history_steps = (session.session().version().get() - initial_version.get()) as usize;
    let edited = session.session().document().snapshot();
    if session.backing().original_bytes() != backing {
        return fail("editing mutated backing");
    }
    let plan = match session.export_plan() {
        Ok(p) => p,
        Err(
            ExportError::Unsupported { .. }
            | ExportError::Restricted
            | ExportError::SignedPackage
            | ExportError::Budget { .. },
        ) => {
            if session
                .session()
                .document()
                .semantic_difference(&edited)
                .is_some()
            {
                return fail("failed export mutated session");
            }
            return ok(4);
        }
        Err(e) => return fail(&format!("unexpected edited export failure: {e}")),
    };
    let second = match session.export_plan() {
        Ok(p) => p,
        Err(_) => return fail("repeated plan failed"),
    };
    if plan.bytes() != second.bytes() || plan.version() != session.session().version() {
        return fail("export determinism/version");
    }
    let restored = match import(plan.bytes(), &budget()) {
        Ok(r) => r,
        Err(_) => return fail("verified plan cannot import"),
    };
    let expected = semantic_body(&edited, session.backing());
    if expected.is_none() || expected != semantic_body(&restored.document, &restored.backing) {
        return fail("supported text roundtrip semantics");
    }
    let original = match Package::read(bytes, &budget()) {
        Ok(p) => p,
        Err(_) => return fail("opened source is not OPC"),
    };
    let output = match Package::read(plan.bytes(), &budget()) {
        Ok(p) => p,
        Err(_) => return fail("output is not OPC"),
    };
    if original.content_types_xml() != output.content_types_xml() {
        return fail("content-types preservation");
    }
    for part in original.parts() {
        if part.name != *session.backing().main_part()
            && output
                .part(part.name.as_str())
                .is_none_or(|p| p.data != part.data)
        {
            return fail("unknown part/relationship byte preservation");
        }
    }
    for _ in 0..history_steps {
        if session.undo().is_err() {
            return fail("undo sequence failed");
        }
    }
    if session
        .session()
        .document()
        .semantic_difference(&before)
        .is_some()
    {
        return fail("undo semantics");
    }
    for _ in 0..history_steps {
        if session.redo().is_err() {
            return fail("redo sequence failed");
        }
    }
    if session
        .session()
        .document()
        .semantic_difference(&edited)
        .is_some()
    {
        return fail("redo semantics");
    }
    ok(9)
}

// Token/run boundaries and imported IDs are not semantics. Compare the
// properties and concatenated text of adjacent equal-format runs. Preserved
// property handles are resolved to the kept bytes, never compared as IDs.
fn semantic_body(
    doc: &joyword_model::Document,
    backing: &joyword_docx::PackageBacking,
) -> Option<Vec<String>> {
    let fragment = |id: Option<joyword_model::PreservedId>| -> Option<String> {
        match id {
            None => Some(String::new()),
            Some(id) => {
                let f = backing.fragment(id)?;
                Some(format!(
                    "{}:{:?}:{}",
                    f.element,
                    f.namespaces,
                    digest(backing.fragment_bytes(id)?)
                ))
            }
        }
    };
    let mut body = Vec::new();
    for block in doc.body().blocks.iter() {
        let p = block.as_paragraph()?;
        if !p.inlines().is_empty() {
            return None;
        }
        let mut props = (*p.props).clone();
        let kept = fragment(props.preserved.take())?;
        let mut mark = (*p.mark_props).clone();
        let mark_kept = fragment(mark.preserved.take())?;
        let mut item = format!("{:?}|{props:?}|{kept}|{mark:?}|{mark_kept}", p.style);
        let text = p.plain_text(|_| unreachable!());
        let mut chars = text.chars();
        let mut previous = String::new();
        let mut content = String::new();
        for run in p.runs() {
            let mut props = (*run.props).clone();
            let kept = fragment(props.preserved.take())?;
            let key = format!("{:?}|{props:?}|{kept}", run.style);
            if !previous.is_empty() && previous != key {
                item.push_str(&format!("|{previous}={content:?}"));
                content.clear();
            }
            content.extend(chars.by_ref().take(run.len));
            previous = key;
        }
        item.push_str(&format!("|{previous}={content:?}"));
        body.push(item);
    }
    Some(body)
}
