//! Three budgeted targets. A panic must crash the process, never become OK.
use joyoffice_opc::{Budget, Package};
use joyword_docx::{DiagnosticKind, Editability, ExportError, ImportError, ReadOnlyReason, import};
use joyword_engine::{Command, Transaction};
use joyword_model::{Bias, Position, ScalarOffset};
use joyword_shell::docx::DocxSession;
use sha2::{Digest, Sha256};
pub const TARGET_SHA: &str = "86c8c04c3d8d0460a7d0e32ac7a733b7d15f72ea";
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
    /// Observable stage facts (sanitized variant names and size buckets only,
    /// never document content or paths). This is a behavioral feedback signal
    /// for corpus-expanding fuzz drivers, not code coverage.
    pub features: Vec<String>,
}
/// Coarse bucket so the feedback vocabulary stays finite.
fn bucket(n: usize) -> &'static str {
    match n {
        0 => "0",
        1 => "1",
        2 => "2",
        3 => "3",
        4..=7 => "4-7",
        8..=15 => "8-15",
        16..=31 => "16-31",
        32..=63 => "32-63",
        64..=127 => "64-127",
        _ => "128+",
    }
}
/// First whitespace token, reduced to a bounded static-looking identifier.
fn feat_token(text: &str) -> String {
    let token = text.split_whitespace().next().unwrap_or("none");
    let mut out = String::new();
    let mut last_dash = false;
    for c in token.chars().take(24) {
        let c = c.to_ascii_lowercase();
        if c.is_ascii_alphanumeric() || c == '_' || c == '-' {
            out.push(c);
            last_dash = false;
        } else if !last_dash && !out.is_empty() {
            out.push('-');
            last_dash = true;
        }
    }
    while out.ends_with('-') {
        out.pop();
    }
    if out.is_empty() { "none".into() } else { out }
}
fn kind_name(kind: &ResultKind) -> &'static str {
    match kind {
        ResultKind::Ok => "ok",
        ResultKind::Rejected => "rejected",
        ResultKind::Invariant => "invariant",
    }
}
fn import_error_name(error: &ImportError) -> &'static str {
    match error {
        ImportError::Package(_) => "package",
        ImportError::MissingMainDocument => "missing-main",
        ImportError::AmbiguousMainDocument => "ambiguous-main",
        ImportError::StrictUnsupported => "strict",
        ImportError::InvalidMainDocumentTarget { .. } => "invalid-main-target",
        ImportError::MissingPart { .. } => "missing-part",
        ImportError::UnexpectedContentType { .. } => "unexpected-content-type",
        ImportError::Xml { .. } => "xml",
        ImportError::UnexpectedElement { .. } => "unexpected-element",
    }
}
fn diag_kind_name(kind: &DiagnosticKind) -> &'static str {
    match kind {
        DiagnosticKind::Package(_) => "package",
        DiagnosticKind::ExternalTarget { .. } => "external-target",
        DiagnosticKind::Preserved { .. } => "preserved",
        DiagnosticKind::ReadOnly(_) => "readonly",
        DiagnosticKind::MissingPart { .. } => "missing-part",
        DiagnosticKind::NotImported { .. } => "not-imported",
        DiagnosticKind::InvalidValue { .. } => "invalid-value",
        DiagnosticKind::StyleSkipped { .. } => "style-skipped",
        DiagnosticKind::SectionDefaults { .. } => "section-defaults",
        DiagnosticKind::PropertiesPreserved { .. } => "properties-preserved",
        DiagnosticKind::HintsSkipped { .. } => "hints-skipped",
        DiagnosticKind::UnpreservedWhitespace => "unpreserved-whitespace",
        DiagnosticKind::EmptyBody => "empty-body",
        DiagnosticKind::MacroProject => "macro-project",
        DiagnosticKind::Image { .. } => "image",
        DiagnosticKind::Model(_) => "model",
        DiagnosticKind::RevisionSubset { .. } => "revision-subset",
        DiagnosticKind::MoveRange { .. } => "move-range",
    }
}
fn read_only_reason_name(reason: &ReadOnlyReason) -> &'static str {
    match reason {
        ReadOnlyReason::DocumentProtection { .. } => "document-protection",
        ReadOnlyReason::WriteProtection => "write-protection",
        ReadOnlyReason::TrackRevisions => "track-revisions",
        ReadOnlyReason::Revisions => "revisions",
        ReadOnlyReason::MustUnderstand { .. } => "must-understand",
        ReadOnlyReason::UnknownNamespace { .. } => "unknown-namespace",
        ReadOnlyReason::ModelInvalid => "model-invalid",
    }
}
/// Features observable from a successfully imported document.
fn import_features(
    editability: &Editability,
    diagnostics: &joyword_docx::Diagnostics,
    paras: usize,
) -> Vec<String> {
    let mut features = vec![
        "import.opened".to_string(),
        format!("import.paras.{}", bucket(paras)),
        format!("import.diag.{}", bucket(diagnostics.len())),
    ];
    if diagnostics.suppressed() > 0 {
        features.push("import.diag.suppressed".into());
    }
    if let Some(severity) = diagnostics.max_severity() {
        features.push(format!("import.sev.{severity:?}").to_ascii_lowercase());
    }
    for item in diagnostics.iter() {
        features.push(format!("import.d.{}", diag_kind_name(&item.kind)));
    }
    if editability.is_unrestricted() {
        features.push("import.edit.unrestricted".into());
    } else {
        for reason in editability.read_only_reasons() {
            features.push(format!("import.ro.{}", read_only_reason_name(reason)));
        }
        if !editability.locked().is_empty() {
            features.push(format!(
                "import.locked.{}",
                bucket(editability.locked().len())
            ));
        }
    }
    features
}
fn ok(features: Vec<String>, checks: usize) -> Report {
    Report {
        kind: ResultKind::Ok,
        reason: String::new(),
        checks,
        features,
    }
}
fn reject(features: Vec<String>, reason: impl ToString) -> Report {
    Report {
        kind: ResultKind::Rejected,
        reason: reason.to_string(),
        checks: 0,
        features,
    }
}
fn fail(features: Vec<String>, reason: &str) -> Report {
    Report {
        kind: ResultKind::Invariant,
        reason: reason.into(),
        checks: 0,
        features,
    }
}
pub fn run(target: &str, bytes: &[u8], seed: u64) -> Report {
    let mut report = run_target(target, bytes, seed);
    report.features.push(format!("target.{target}"));
    report
        .features
        .push(format!("status.{}", kind_name(&report.kind)));
    report
        .features
        .push(format!("reason.{}", feat_token(&report.reason)));
    report.features.sort();
    report.features.dedup();
    report
}
fn run_target(target: &str, bytes: &[u8], seed: u64) -> Report {
    if bytes.len() > MAX_INPUT {
        return reject(vec![], "input byte ceiling");
    }
    match target {
        "opc" => match Package::read(bytes, &budget()) {
            Ok(p) => {
                let entries = p.parts().count();
                let total = p.parts().map(|part| part.data.len() as u64).sum::<u64>();
                let features = vec![
                    "opc.opened".into(),
                    format!("opc.entries.{}", bucket(entries)),
                    format!("opc.total.{}", bucket(total as usize)),
                ];
                if total > budget().max_total_bytes {
                    return fail(features, "OPC inflated budget");
                }
                ok(features, 1)
            }
            Err(e) => reject(vec!["opc.refused".into()], e),
        },
        "import" => match (import(bytes, &budget()), import(bytes, &budget())) {
            (Ok(a), Ok(b)) => {
                if a.document.semantic_difference(&b.document).is_some()
                    || a.editability != b.editability
                {
                    return fail(vec!["import.opened".into()], "import determinism");
                }
                if a.backing.original_bytes() != bytes {
                    return fail(vec!["import.opened".into()], "import exact bytes");
                }
                let paras = a
                    .document
                    .body()
                    .blocks
                    .iter()
                    .filter(|b| b.as_paragraph().is_some())
                    .count();
                ok(import_features(&a.editability, &a.diagnostics, paras), 2)
            }
            (Err(a), Err(b)) if a == b => reject(
                vec![format!("import.error.{}", import_error_name(&a))],
                a.to_string(),
            ),
            (Err(a), _) | (_, Err(a)) => fail(
                vec![
                    "import.opened".into(),
                    format!("import.error.{}", import_error_name(&a)),
                ],
                "nondeterministic import outcome",
            ),
        },
        "roundtrip" => roundtrip(bytes, seed),
        _ => fail(vec![], "unknown target"),
    }
}
fn roundtrip(bytes: &[u8], seed: u64) -> Report {
    let mut session = match DocxSession::open(bytes, &budget()) {
        Ok(s) => s,
        Err(e) => return reject(vec!["rt.open.refused".into()], e),
    };
    let unchanged = match session.export_plan() {
        Ok(p) => p,
        Err(e) => return fail(vec!["rt.opened".into()], &format!("unchanged plan: {e}")),
    };
    if unchanged.bytes() != bytes {
        return fail(vec!["rt.opened".into()], "unchanged package copy");
    }
    let Some(p) = session
        .session()
        .document()
        .body()
        .blocks
        .iter()
        .find_map(|b| b.as_paragraph())
    else {
        return reject(vec!["rt.opened".into()], "no paragraph for text target");
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
            return fail(vec!["rt.opened".into()], "rejected edit is not atomic");
        }
        return ok(vec!["rt.opened".into(), "rt.edit.refused".into()], 4);
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
            return fail(
                vec!["rt.edit.applied".into()],
                "generated legal text sequence rejected",
            );
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
            return fail(
                vec!["rt.edit.applied".into()],
                "independent edit sequence text model",
            );
        }
    }
    let history_steps = (session.session().version().get() - initial_version.get()) as usize;
    let edited = session.session().document().snapshot();
    if session.backing().original_bytes() != backing {
        return fail(vec!["rt.edit.applied".into()], "editing mutated backing");
    }
    let export_error_feature = |e: &ExportError| match e {
        ExportError::Unsupported { .. } => "rt.export.unsupported".to_string(),
        ExportError::Restricted => "rt.export.restricted".to_string(),
        ExportError::SignedPackage => "rt.export.signed".to_string(),
        ExportError::Budget { .. } => "rt.export.budget".to_string(),
        ExportError::Import(_) => "rt.export.import".to_string(),
        ExportError::Write(_) => "rt.export.write".to_string(),
        ExportError::RoundTripMismatch => "rt.export.roundtrip-mismatch".to_string(),
    };
    let plan = match session.export_plan() {
        Ok(p) => p,
        Err(
            e @ (ExportError::Unsupported { .. }
            | ExportError::Restricted
            | ExportError::SignedPackage
            | ExportError::Budget { .. }),
        ) => {
            if session
                .session()
                .document()
                .semantic_difference(&edited)
                .is_some()
            {
                return fail(
                    vec!["rt.edit.applied".into()],
                    "failed export mutated session",
                );
            }
            return ok(
                vec![
                    "rt.edit.applied".into(),
                    export_error_feature(&e),
                    "rt.export.refused".into(),
                ],
                4,
            );
        }
        Err(e) => {
            return fail(
                vec!["rt.edit.applied".into()],
                &format!("unexpected edited export failure: {e}"),
            );
        }
    };
    let second = match session.export_plan() {
        Ok(p) => p,
        Err(_) => return fail(vec!["rt.edit.applied".into()], "repeated plan failed"),
    };
    if plan.bytes() != second.bytes() || plan.version() != session.session().version() {
        return fail(vec!["rt.edit.applied".into()], "export determinism/version");
    }
    let restored = match import(plan.bytes(), &budget()) {
        Ok(r) => r,
        Err(_) => {
            return fail(
                vec!["rt.edit.applied".into()],
                "verified plan cannot import",
            );
        }
    };
    let expected = semantic_body(&edited, session.backing());
    if expected.is_none() || expected != semantic_body(&restored.document, &restored.backing) {
        return fail(
            vec!["rt.edit.applied".into()],
            "supported text roundtrip semantics",
        );
    }
    let original = match Package::read(bytes, &budget()) {
        Ok(p) => p,
        Err(_) => return fail(vec!["rt.edit.applied".into()], "opened source is not OPC"),
    };
    let output = match Package::read(plan.bytes(), &budget()) {
        Ok(p) => p,
        Err(_) => return fail(vec!["rt.edit.applied".into()], "output is not OPC"),
    };
    if original.content_types_xml() != output.content_types_xml() {
        return fail(
            vec!["rt.edit.applied".into(), "rt.reimport.ok".into()],
            "content-types preservation",
        );
    }
    let mut preserved = 0usize;
    for part in original.parts() {
        if part.name != *session.backing().main_part() {
            preserved += 1;
            if output
                .part(part.name.as_str())
                .is_none_or(|p| p.data != part.data)
            {
                return fail(
                    vec![
                        "rt.edit.applied".into(),
                        "rt.reimport.ok".into(),
                        format!("rt.preserved.{}", bucket(preserved)),
                    ],
                    "unknown part/relationship byte preservation",
                );
            }
        }
    }
    for _ in 0..history_steps {
        if session.undo().is_err() {
            return fail(vec!["rt.edit.applied".into()], "undo sequence failed");
        }
    }
    if session
        .session()
        .document()
        .semantic_difference(&before)
        .is_some()
    {
        return fail(vec!["rt.edit.applied".into()], "undo semantics");
    }
    for _ in 0..history_steps {
        if session.redo().is_err() {
            return fail(vec!["rt.edit.applied".into()], "redo sequence failed");
        }
    }
    if session
        .session()
        .document()
        .semantic_difference(&edited)
        .is_some()
    {
        return fail(vec!["rt.edit.applied".into()], "redo semantics");
    }
    ok(
        vec![
            "rt.edit.applied".into(),
            "rt.export.planned".into(),
            "rt.reimport.ok".into(),
            "rt.undo.redo.ok".into(),
            format!("rt.preserved.{}", bucket(preserved)),
            format!("rt.steps.{}", bucket(history_steps)),
        ],
        9,
    )
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
