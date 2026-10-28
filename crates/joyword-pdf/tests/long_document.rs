//! T500: a mixed multi-section document — portrait body with a cross-page
//! repeating-header table, a landscape section opening with a page-anchored
//! wrap-square picture, and an even-page section of two separated columns —
//! renders to PDF from the same PageScene the screen consumes. Every run in
//! the scene has a PDF text matrix within 0.01 pt of its origin.

use std::io::Read;
use std::sync::Arc;

use joyword_foundation::Emu;
use joyword_layout::{
    FaceSet, FeatureSet, LayoutCancellation, LayoutLimits, LayoutSnapshot, LoadedFont,
    SingleFontProfile, layout_document, to_points,
};
use joyword_model::{
    AnchorWrap, Block, BlockSeq, Cell, CellBorders, Document, EqualColumns, ImageAnchor,
    InlineKind, InlineObject, Measure, PageSetup, Paragraph, Row, SectionBreak, SquareWrap, Table,
    VerticalMerge, WrapSide, sample_png_1x1,
};

use joyword_pdf::render;

fn pt(value: i64) -> Emu {
    Emu::new(value * Emu::PER_POINT)
}

fn font() -> LoadedFont {
    LoadedFont::from_bytes(
        Arc::from(
            include_bytes!("../../../tests/fixtures/fonts/joyoffice-test-serif.ttf").as_slice(),
        ),
        0,
    )
    .unwrap()
}

fn profile() -> SingleFontProfile {
    SingleFontProfile {
        size: pt(12),
        leading: pt(18),
        baseline: pt(14),
        features: FeatureSet::default(),
    }
}

fn a4_page() -> PageSetup {
    let mut page = PageSetup::a4_zh_cn();
    page.left = Measure::Twips(1440);
    page.right = Measure::Twips(1440);
    page.top = Measure::Twips(1440);
    page.bottom = Measure::Twips(1440);
    page.header = Measure::Twips(720);
    page.footer = Measure::Twips(720);
    page.gutter = Measure::Twips(0);
    page
}

fn paragraph_with(document: &mut Document, text: &str) -> Paragraph {
    let mut paragraph = Paragraph::new(document.ids().node());
    paragraph.push_text(text, None, Arc::default()).unwrap();
    paragraph
}

fn close_section(document: &mut Document, last_block: usize) {
    let boundary = document.body().blocks.get(last_block).unwrap().id();
    let mut next = document.sections().last().unwrap().clone();
    next.id = document.ids().node();
    next.last_block = None;
    next.title_page = None;
    next.headers.clear();
    next.footers.clear();
    let closed = document.sections().len() - 1;
    document.sections_mut()[closed].last_block = Some(boundary);
    document.sections_mut().push(next);
}

fn cell(document: &mut Document, text: &str) -> Cell {
    Cell {
        id: document.ids().node(),
        grid_span: 1,
        vertical_merge: VerticalMerge::None,
        width: Some(Measure::Twips(4000)),
        borders: CellBorders::default(),
        shading: None,
        margin: None,
        vertical_align: None,
        blocks: BlockSeq::from_vec(vec![paragraph_with(document, text).into()]),
        preserved: None,
        props_revision: None,
    }
}

/// The mixed multi-section sample of the T500 long-document tests, at a size
/// of a handful of pages: a table that crosses a page with a repeating
/// header, one landscape section with a floating picture, and a final
/// even-page section of two separated columns.
fn mixed_document(section_paragraphs: usize, table_rows: usize) -> Document {
    let mut document = Document::new(a4_page());
    let mut blocks: Vec<Block> = Vec::new();
    for index in 0..section_paragraphs {
        blocks.push(
            paragraph_with(
                &mut document,
                &format!(
                    "第{index:04}段：混合分节样例正文，段落占据两行以上保持分页稳定，用于几何对照。"
                ),
            )
            .into(),
        );
    }
    let mut rows = vec![Row {
        id: document.ids().node(),
        grid_before: 0,
        grid_after: 0,
        cells: vec![
            cell(&mut document, "表头列甲"),
            cell(&mut document, "表头列乙"),
        ],
        header: true,
        cant_split: false,
        height: None,
        preserved: None,
        props_revision: None,
    }];
    for index in 0..table_rows {
        rows.push(Row {
            id: document.ids().node(),
            grid_before: 0,
            grid_after: 0,
            cells: vec![
                cell(&mut document, &format!("数据行{index}左列")),
                cell(&mut document, &format!("数据行{index}右列")),
            ],
            header: false,
            cant_split: false,
            height: None,
            preserved: None,
            props_revision: None,
        });
    }
    blocks.push(
        Table {
            id: document.ids().node(),
            grid: vec![Measure::Twips(4000); 2],
            rows,
            style: None,
            width: None,
            layout: None,
            alignment: None,
            indent: None,
            look: None,
            cell_margin: None,
            preserved: None,
            props_revision: None,
            source: None,
        }
        .into(),
    );
    let section0_last = blocks.len() - 1;

    let resource = document.ids().resource();
    document
        .resources_mut()
        .insert(resource, "image/png", Arc::from(sample_png_1x1()), None);
    let mut picture = Paragraph::new(document.ids().node());
    picture.push_inline(
        InlineObject {
            id: document.ids().node(),
            kind: InlineKind::Image {
                resource,
                width: Measure::Emu(pt(72).raw()),
                height: Measure::Emu(pt(72).raw()),
                anchor: Some(ImageAnchor {
                    x: Measure::Emu(pt(312).raw()),
                    y: Measure::Emu(pt(180).raw()),
                    wrap: AnchorWrap::Square(SquareWrap {
                        top: 0,
                        bottom: 0,
                        left: 0,
                        right: 0,
                        side: WrapSide::Larger,
                    }),
                }),
            },
            source: None,
        },
        None,
        Arc::default(),
    );
    picture
        .push_text(
            "带浮动图片的段落，文字绕排在锚定矩形两侧。",
            None,
            Arc::default(),
        )
        .unwrap();
    blocks.push(picture.into());
    for index in section_paragraphs..2 * section_paragraphs {
        blocks.push(
            paragraph_with(
                &mut document,
                &format!("横向{index:04}段：横排分节保持两行以上的正文，与纵向分节共用一次布局。"),
            )
            .into(),
        );
    }
    let section1_last = blocks.len() - 1;
    for index in 2 * section_paragraphs..3 * section_paragraphs {
        blocks.push(
            paragraph_with(
                &mut document,
                &format!("分栏{index:04}段：双栏分节的正文段落，两栏之间绘制栏分隔线。"),
            )
            .into(),
        );
    }
    let body = document.body_id();
    document.story_mut(body).unwrap().blocks = BlockSeq::from_vec(blocks);
    close_section(&mut document, section0_last);
    close_section(&mut document, section1_last);
    let portrait = a4_page();
    let landscape = &mut document.sections_mut()[1];
    landscape.break_kind = SectionBreak::NextPage;
    landscape.page = PageSetup {
        width: portrait.height,
        height: portrait.width,
        ..portrait
    };
    let columns = &mut document.sections_mut()[2];
    columns.break_kind = SectionBreak::EvenPage;
    columns.columns = Some(EqualColumns {
        count: 2,
        space: 720,
    });
    columns.column_separator = true;
    document
}

fn layout(document: &Document) -> LayoutSnapshot {
    layout_document(
        &document.snapshot(),
        FaceSet::uniform(&font()),
        profile(),
        LayoutLimits::default(),
        &LayoutCancellation::default(),
    )
    .unwrap()
}

fn near(actual: f64, expected: f64) -> bool {
    (actual - expected).abs() <= 0.01
}

fn inflate_streams(pdf: &[u8]) -> Vec<Vec<u8>> {
    let mut streams = Vec::new();
    let mut rest = pdf;
    while let Some(index) = rest.windows(6).position(|window| window == b"stream") {
        let after = &rest[index + 6..];
        let data = if after.starts_with(b"\r\n") {
            &after[2..]
        } else if let Some(stripped) = after.strip_prefix(b"\n") {
            stripped
        } else {
            rest = &rest[index + 6..];
            continue;
        };
        let mut decoder = flate2::read::ZlibDecoder::new(data);
        let mut inflated = Vec::new();
        if decoder.read_to_end(&mut inflated).is_ok() && !inflated.is_empty() {
            streams.push(inflated);
        }
        rest = if data.is_empty() { &[] } else { &data[1..] };
    }
    streams
}

fn text_matrices(pdf: &[u8]) -> Vec<(f64, f64)> {
    let mut matrices = Vec::new();
    for stream in inflate_streams(pdf) {
        let text = String::from_utf8_lossy(&stream);
        let tokens: Vec<&str> = text.split_whitespace().collect();
        for (index, token) in tokens.iter().enumerate() {
            let operator = *token == "Tm"
                || token.starts_with("Tm[")
                || token.starts_with("Tm(")
                || token.starts_with("Tm/");
            if !operator || index < 6 {
                continue;
            }
            let numbers: Vec<f64> = tokens[index - 6..index]
                .iter()
                .filter_map(|token| token.parse().ok())
                .collect();
            if numbers.len() == 6 {
                matrices.push((numbers[4], numbers[5]));
            }
        }
    }
    matrices
}

#[test]
fn a_mixed_section_document_shares_one_geometry_with_the_pdf() {
    let document = mixed_document(24, 30);
    let snapshot = layout(&document);
    assert!(
        snapshot.pages().len() >= 4,
        "{} pages",
        snapshot.pages().len()
    );
    // Portrait, landscape and separated-column pages are all present, the
    // repeating header spans pages, and the anchored picture is painted.
    let geometries = snapshot.page_geometry();
    assert!(
        geometries
            .iter()
            .any(|geometry| geometry.width.raw() < geometry.height.raw())
    );
    assert!(
        geometries
            .iter()
            .any(|geometry| geometry.width.raw() > geometry.height.raw())
    );
    assert!(snapshot.pages().iter().any(|page| {
        page.rects
            .iter()
            .any(|rect| rect.width == Emu::new(Emu::PER_POINT))
    }));
    assert!(
        snapshot
            .pages()
            .iter()
            .any(|page| page.runs.iter().any(|run| run.text.contains("表头列甲")))
    );
    assert!(snapshot.pages().iter().any(|page| !page.images.is_empty()));

    // The PDF renders from the same scene: every run origin has a matching
    // text matrix within 0.01 pt.
    let pdf = render(snapshot.pages(), &[font()]).unwrap();
    assert!(pdf.starts_with(b"%PDF"));
    let matrices = text_matrices(&pdf);
    assert!(!matrices.is_empty());
    let mut checked = 0usize;
    for page in snapshot.pages() {
        for run in &page.runs {
            if run.glyphs.is_empty() {
                continue;
            }
            let x = to_points(run.origin_x);
            let y = to_points(run.baseline_y);
            assert!(
                matrices.iter().any(|(mx, my)| near(*mx, x) && near(*my, y)),
                "no PDF text matrix within 0.01 pt of ({x}, {y})"
            );
            checked += 1;
        }
    }
    assert!(checked >= 100, "{checked} runs compared");
}
