//! PPTX export: [`Presentation`] → package.

pub(crate) mod chart;
pub(crate) mod dml;
mod shapes;
mod timing;

use std::collections::HashMap;
use std::sync::Arc;

use joydeck_geom::Xfrm;
use joydeck_model::text::{ListStyle, Paragraph, TextBody};
use joydeck_model::theme::Theme;
use joydeck_model::{
    Background, Layout, LayoutId, MediaId, PhType, Placeholder, Presentation, Shape, ShapeId,
    Slide, SlideId, defaults,
};

use crate::opc::{self, NS_A, NS_P, NS_P14, NS_P188, NS_R, PackageOut, RelsOut};
use crate::xml::{A, W};
use crate::{PptxError, master_part};

pub(crate) const CT_PRES: &str =
    "application/vnd.openxmlformats-officedocument.presentationml.presentation.main+xml";
pub(crate) const CT_TEMPLATE: &str =
    "application/vnd.openxmlformats-officedocument.presentationml.template.main+xml";
pub(crate) const CT_SHOW: &str =
    "application/vnd.openxmlformats-officedocument.presentationml.slideshow.main+xml";
const CT_SLIDE: &str = "application/vnd.openxmlformats-officedocument.presentationml.slide+xml";
pub(super) const CT_LAYOUT: &str =
    "application/vnd.openxmlformats-officedocument.presentationml.slideLayout+xml";
pub(super) const CT_MASTER: &str =
    "application/vnd.openxmlformats-officedocument.presentationml.slideMaster+xml";
const CT_NOTES_MASTER: &str =
    "application/vnd.openxmlformats-officedocument.presentationml.notesMaster+xml";
const CT_HANDOUT_MASTER: &str =
    "application/vnd.openxmlformats-officedocument.presentationml.handoutMaster+xml";
const CT_NOTES: &str =
    "application/vnd.openxmlformats-officedocument.presentationml.notesSlide+xml";
pub(super) const CT_THEME: &str = "application/vnd.openxmlformats-officedocument.theme+xml";
const CT_PRES_PROPS: &str =
    "application/vnd.openxmlformats-officedocument.presentationml.presProps+xml";
const CT_VIEW_PROPS: &str =
    "application/vnd.openxmlformats-officedocument.presentationml.viewProps+xml";
const CT_TABLE_STYLES: &str =
    "application/vnd.openxmlformats-officedocument.presentationml.tableStyles+xml";
const CT_TAGS: &str = "application/vnd.openxmlformats-officedocument.presentationml.tags+xml";
const CT_CHART: &str = "application/vnd.openxmlformats-officedocument.drawingml.chart+xml";
const CT_CHARTEX: &str = "application/vnd.ms-office.chartex+xml";
const RT_CHARTEX: &str = "http://schemas.microsoft.com/office/2014/relationships/chartEx";
const CT_COMMENTS: &str =
    "application/vnd.openxmlformats-officedocument.presentationml.comments+xml";
const CT_AUTHORS: &str =
    "application/vnd.openxmlformats-officedocument.presentationml.commentAuthors+xml";
/// Modern threaded comments (read by PowerPoint 2021+ and DeckCraft; the flattened legacy
/// part next to it serves older readers).
pub(crate) const CT_COMMENTS_MODERN: &str = "application/vnd.ms-powerpoint.comments+xml";
const CT_AUTHORS_MODERN: &str = "application/vnd.ms-powerpoint.authors+xml";
const RT_COMMENTS_MODERN: &str =
    "http://schemas.microsoft.com/office/2018/10/relationships/comments";
const RT_AUTHORS_MODERN: &str = "http://schemas.microsoft.com/office/2018/10/relationships/authors";
const CT_CORE: &str = "application/vnd.openxmlformats-package.core-properties+xml";
const CT_APP: &str = "application/vnd.openxmlformats-officedocument.extended-properties+xml";

/// Export state.
pub struct Exp<'a> {
    error: Option<PptxError>,
    pub p: &'a Presentation,
    pub pkg: PackageOut,
    media: crate::media_part::Writer,
    n_chart: u32,
    n_diagram: u32,
    n_tags: u32,
    n_chartex: u32,
    /// Parts kept from an unchanged chart (`ppt/…/keptN.ext`).
    n_kept: u32,
    pub slide_parts: HashMap<SlideId, String>,
    pub table_styles: Vec<String>,
}

/// A part being written: its name and relationships.
pub struct Out {
    pub name: String,
    pub rels: RelsOut,
    field_seq: usize,
}

impl Out {
    pub(super) fn new(name: &str) -> Self {
        Out {
            name: name.to_string(),
            rels: RelsOut::default(),
            field_seq: 0,
        }
    }

    pub(crate) fn next_field_id(&mut self) -> String {
        self.field_seq += 1;
        crate::tables::guid(&format!("{}:field:{}", self.name, self.field_seq))
    }
}

impl Exp<'_> {
    pub fn media_rel(&mut self, owner: &mut Out, id: MediaId, kind: &str) -> Option<String> {
        self.media
            .relationship(self.p, &mut self.pkg, owner, id, kind, true)
    }

    pub fn media_rel_new(&mut self, owner: &mut Out, id: MediaId, kind: &str) -> Option<String> {
        self.media
            .relationship(self.p, &mut self.pkg, owner, id, kind, false)
    }

    pub fn placeholder_rel(&mut self, owner: &mut Out) -> String {
        self.media.missing(&mut self.pkg, owner)
    }

    /// The number of the next SmartArt graphic's parts.
    pub fn next_diagram(&mut self) -> u32 {
        self.n_diagram += 1;
        self.n_diagram
    }
    /// A new `ppt/tags/tagN.xml` part holding `tags`, related from `o`.
    pub fn tags_rel(&mut self, o: &mut Out, tags: &Tags) -> String {
        self.n_tags += 1;
        let name = format!("ppt/tags/tag{}.xml", self.n_tags);
        let mut w = W::new();
        root_open(&mut w, "p:tagLst", A::new());
        for (n, v) in tags.iter().take(10_000) {
            w.empty("p:tag", A::new().a("name", n).a("val", v));
        }
        w.close("p:tagLst");
        self.pkg.xml_part(&name, CT_TAGS, w.finish());
        o.rels
            .add_new("tags", &opc::relative(&o.name, &name), false)
    }
    /// Write the actual ChartEx part or use the classic path for other chart types.
    pub fn chartex_rel(&mut self, owner: &mut Out, chart: &joydeck_model::Chart) -> Option<String> {
        use crate::{
            chart_sidecars, chart_style_parts, chart_workbook::ChartWorkbook, chartex_write,
        };
        if !chart.kind.is_chartex() {
            return None;
        }
        let original = chartex_write::retained(chart);
        let count = chartex_write::series_count(chart);
        if original.is_none() && count == 0 {
            return None;
        }
        self.n_chartex += 1;
        let name = format!("ppt/charts/chartEx{}.xml", self.n_chartex);
        let mut relationships = chart_sidecars::write(
            &mut self.pkg,
            &name,
            chart,
            &mut self.n_kept,
            original.is_none(),
        );
        let data = if let Some(original) = original {
            original
        } else {
            let workbook = ChartWorkbook::new(chart, true, count);
            let link = workbook.as_ref().and_then(|sheet| {
                self.attach_workbook(
                    sheet,
                    &name,
                    &format!("joydeck-chartex-{}.xlsx", self.n_chartex),
                    &mut relationships,
                )
            });
            let binding = workbook.as_ref().zip(link.as_deref());
            chartex_write::encode(chart, &name, binding)?
        };
        chart_style_parts::ensure(&mut self.pkg, &name, self.n_chartex, &mut relationships);
        self.pkg.rels(&name, &relationships);
        self.pkg.xml_part(&name, CT_CHARTEX, data);
        Some(
            owner
                .rels
                .add_new(RT_CHARTEX, &opc::relative(&owner.name, &name), false),
        )
    }

    pub fn chart_rel(&mut self, owner: &mut Out, model: &joydeck_model::Chart) -> String {
        self.n_chart += 1;
        let name = format!("ppt/charts/chart{}.xml", self.n_chart);
        let original = chart::retained(model);
        let mut relationships = crate::chart_sidecars::write(
            &mut self.pkg,
            &name,
            model,
            &mut self.n_kept,
            original.is_none(),
        );
        let data = if let Some(original) = original {
            original
        } else {
            let workbook = (!model.kind.is_chartex())
                .then(|| {
                    crate::chart_workbook::ChartWorkbook::new(model, false, model.series.len())
                })
                .flatten();
            let link = workbook.as_ref().and_then(|sheet| {
                self.attach_workbook(
                    sheet,
                    &name,
                    &format!("joydeck-chart-{}.xlsx", self.n_chart),
                    &mut relationships,
                )
            });
            chart::build(model, workbook.as_ref().zip(link.as_deref()))
        };
        self.pkg.rels(&name, &relationships);
        self.pkg.xml_part(&name, CT_CHART, data);
        owner
            .rels
            .add_new("chart", &opc::relative(&owner.name, &name), false)
    }

    fn attach_workbook(
        &mut self,
        sheet: &crate::chart_workbook::ChartWorkbook<'_>,
        chart: &str,
        filename: &str,
        relationships: &mut RelsOut,
    ) -> Option<String> {
        match sheet.attach(&mut self.pkg, chart, filename, relationships) {
            Ok(id) => Some(id),
            Err(error) => {
                self.error = Some(error);
                None
            }
        }
    }
}

/// `p:custDataLst` pointing at a tags part, when there are tags.
pub fn cust_data(w: &mut W, x: &mut Exp, o: &mut Out, tags: &Tags) {
    if tags.is_empty() {
        return;
    }
    let id = x.tags_rel(o, tags);
    w.open0("p:custDataLst");
    w.empty("p:tags", A::new().a("r:id", id));
    w.close("p:custDataLst");
}

pub(super) fn root_open(w: &mut W, tag: &str, extra: A) {
    let mut a = A::new()
        .a("xmlns:a", NS_A)
        .a("xmlns:r", NS_R)
        .a("xmlns:p", NS_P);
    a.v.extend(extra.v);
    w.open(tag, a);
}

type Tags = [(String, String)];

pub(super) fn csld(
    w: &mut W,
    x: &mut Exp,
    o: &mut Out,
    name: &str,
    bg: Option<&Background>,
    list: &[Shape],
    tags: &Tags,
) -> shapes::IdMap {
    w.open(
        "p:cSld",
        A::new().o("name", (!name.is_empty()).then_some(name)),
    );
    if let Some(bg) = bg {
        master_part::write_background(w, x, o, bg);
    }
    w.open0("p:spTree");
    shapes::tree_header(w);
    let mut ids = shapes::IdMap::build(list);
    {
        let mut cx = shapes::Ctx {
            ids: &mut ids,
            written: Default::default(),
        };
        shapes::shapes(w, x, o, &mut cx, list, 0);
    }
    w.close("p:spTree");
    cust_data(w, x, o, tags);
    w.close("p:cSld");
    ids
}

pub(super) fn ext_list(w: &mut W, raw: Option<&str>) {
    let Some(raw) = raw.filter(|r| !r.trim().is_empty()) else {
        return;
    };
    // Only keep well-formed extension elements.
    let Ok(d) = crate::xml::parse(format!("<x>{raw}</x>").as_bytes()) else {
        return;
    };
    let exts: Vec<String> = d
        .root
        .elements()
        .filter(|e| e.is("ext") && e.has_attr("uri"))
        .map(|e| e.to_xml())
        .collect();
    if exts.is_empty() {
        return;
    }
    w.open0("p:extLst");
    for e in exts {
        w.raw(&e);
    }
    w.close("p:extLst");
}

/// Strip an `<p:extLst>` wrapper (masters/layouts keep the whole element).
pub(super) fn ext_children(raw: Option<&str>) -> Option<String> {
    let raw = raw?;
    let d = crate::xml::parse(raw.as_bytes()).ok()?;
    if d.root.is("extLst") {
        let ns = d.ns_decls();
        return Some(
            d.root
                .elements()
                .map(|e| crate::read::keep_xml(e, &ns))
                .collect(),
        );
    }
    Some(raw.to_string())
}

/// The slide's extension list, plus the modern-comments anchor when the slide has a threaded
/// comment part: the rel reference is how PowerPoint finds it.
fn slide_ext_list(w: &mut W, raw: Option<&str>, comment_rel: Option<&str>) {
    let mut exts: Vec<String> = raw
        .filter(|r| !r.trim().is_empty())
        .and_then(|raw| crate::xml::parse(format!("<x>{raw}</x>").as_bytes()).ok())
        .map(|d| {
            d.root
                .elements()
                .filter(|e| e.is("ext") && e.has_attr("uri"))
                .map(|e| e.to_xml())
                .collect()
        })
        .unwrap_or_default();
    if let Some(rid) = comment_rel {
        let mut e = W::frag();
        e.open("p:ext", A::new().a("uri", crate::COMMENTS_EXT_URI));
        e.empty(
            "p188:commentRel",
            A::new().a("xmlns:p188", NS_P188).a("r:id", rid),
        );
        e.close("p:ext");
        exts.push(e.s);
    }
    if exts.is_empty() {
        return;
    }
    w.open0("p:extLst");
    for e in exts {
        w.raw(&e);
    }
    w.close("p:extLst");
}

fn notes_master_shapes(size: joydeck_geom::Size) -> Vec<Shape> {
    let (w, h) = (size.width, size.height);
    let mk =
        |id: u32, name: &str, kind: PhType, idx: u32, x: f64, y: f64, ww: f64, hh: f64| Shape {
            id: ShapeId(id),
            name: name.into(),
            xfrm: Some(Xfrm::new(x, y, ww, hh)),
            ph: Some(Placeholder {
                kind,
                idx,
                size: (kind != PhType::SlideImage).then(|| "quarter".to_string()),
                ..Default::default()
            }),
            text: (kind != PhType::SlideImage).then(TextBody::default),
            ..Default::default()
        };
    let mut img = mk(
        4,
        "Slide Image Placeholder 3",
        PhType::SlideImage,
        2,
        w * 0.125,
        h * 0.125,
        w * 0.75,
        h * 0.375,
    );
    img.fill = Some(joydeck_model::Fill::None);
    img.line = Some(joydeck_model::Line::solid(
        joydeck_model::ColorRef::rgb(joydeck_color::Rgba::BLACK),
        1.0,
    ));
    let mut body = mk(
        5,
        "Notes Placeholder 4",
        PhType::Body,
        3,
        w * 0.1,
        h * 0.53,
        w * 0.8,
        h * 0.36,
    );
    if let Some(t) = body.text.as_mut() {
        t.paragraphs = vec![Paragraph::new("Click to edit Master text styles")];
    }
    vec![
        mk(
            2,
            "Header Placeholder 1",
            PhType::Header,
            0,
            0.0,
            0.0,
            w * 0.43,
            h * 0.05,
        ),
        mk(
            3,
            "Date Placeholder 2",
            PhType::Date,
            1,
            w * 0.57,
            0.0,
            w * 0.43,
            h * 0.05,
        ),
        img,
        body,
        mk(
            6,
            "Footer Placeholder 5",
            PhType::Footer,
            4,
            0.0,
            h * 0.95,
            w * 0.43,
            h * 0.05,
        ),
        mk(
            7,
            "Slide Number Placeholder 6",
            PhType::SlideNum,
            5,
            w * 0.57,
            h * 0.95,
            w * 0.43,
            h * 0.05,
        ),
    ]
}

/// What the notes master should be on save. A master we read is written back, including its
/// header, footer, date and slide-number text. A deck that has never had one gets the placeholders
/// we generate.
struct NotesMasterWrite {
    shapes: Vec<Shape>,
    background: Option<Background>,
    color_map: Vec<(String, String)>,
    style: ListStyle,
    theme: Option<Theme>,
}

fn notes_master_write(p: &Presentation) -> Option<NotesMasterWrite> {
    let nm = p.notes_master.as_deref();
    let kept = nm.filter(|m| !m.shapes.is_empty());
    let has_notes = p.slides.iter().any(|s| !s.notes.is_empty());
    if !has_notes && nm.is_none() {
        return None;
    }
    let default_bg = || Background::Ref {
        idx: 1001,
        color: joydeck_model::ColorRef::scheme(joydeck_color::SchemeSlot::Bg1),
    };
    Some(match kept {
        Some(m) => NotesMasterWrite {
            shapes: m.shapes.clone(),
            background: Some(m.background.clone().unwrap_or_else(default_bg)),
            color_map: m.color_map.clone(),
            style: if m.body_style.is_empty() {
                notes_style()
            } else {
                m.body_style.clone()
            },
            theme: Some(m.theme.clone()),
        },
        // A master without placeholders of its own gets PowerPoint's defaults, keeping its
        // background and colour map.
        None => NotesMasterWrite {
            shapes: notes_master_shapes(p.notes_size),
            background: Some(
                nm.and_then(|m| m.background.clone())
                    .unwrap_or_else(default_bg),
            ),
            color_map: nm.map(|m| m.color_map.clone()).unwrap_or_default(),
            style: notes_style(),
            theme: None,
        },
    })
}

fn notes_style() -> ListStyle {
    let mut ls = joydeck_model::text::ListStyle::default();
    for i in 0..9u8 {
        ls.set(
            i,
            joydeck_model::text::LevelStyle {
                para: joydeck_model::text::ParaProps {
                    margin_left: Some(36.0 * i as f64),
                    align: Some(joydeck_model::text::Align::Left),
                    ..Default::default()
                },
                run: joydeck_model::text::RunProps {
                    size: Some(12.0),
                    font: Some("+mn-lt".into()),
                    fill: Some(joydeck_model::Fill::solid(joydeck_model::ColorRef::scheme(
                        joydeck_color::SchemeSlot::Tx1,
                    ))),
                    ..Default::default()
                },
            },
        );
    }
    ls
}

/// A plausible xsd:dateTime (`YYYY-MM-DD[THH:MM:SS(.frac)?(Z|±HH:MM)?]`): the ranges are
/// checked, so a model value like `2025-13-45T99:99` falls back to the default instead of
/// reaching a required `dt`/`created` attribute.
pub(super) fn valid_date(s: &str) -> bool {
    let b = s.as_bytes();
    let n = |r: std::ops::Range<usize>| s.get(r).and_then(|v| v.parse::<u32>().ok());
    let (Some(_), Some(mo), Some(d)) = (n(0..4), n(5..7), n(8..10)) else {
        return false;
    };
    if b.get(4) != Some(&b'-')
        || b.get(7) != Some(&b'-')
        || !(1..=12).contains(&mo)
        || !(1..=31).contains(&d)
    {
        return false;
    }
    match b.get(10) {
        None => true,
        Some(b'T') => match (n(11..13), n(14..16), n(17..19)) {
            (Some(h), Some(mi), Some(sec)) => {
                h <= 23
                    && mi <= 59
                    && sec <= 60
                    && b.get(13) == Some(&b':')
                    && b.get(16) == Some(&b':')
            }
            _ => false,
        },
        Some(_) => false,
    }
}

/// A copy of the presentation with every slide pointing at a layout that exists.
fn sanitized(p: &Presentation) -> std::borrow::Cow<'_, Presentation> {
    let mut need = p.masters.is_empty() || p.masters.iter().any(|m| m.layouts.is_empty());
    need |= p.slides.iter().any(|s| p.layout(s.layout).is_none());
    if !need {
        return std::borrow::Cow::Borrowed(p);
    }
    let mut q = p.clone();
    if q.masters.is_empty() {
        let (m, next) = defaults::build_master(q.slide_size, Default::default(), q.next_id.max(1));
        q.next_id = next;
        q.masters.push(Arc::new(m));
    }
    for m in q.masters.iter_mut() {
        if m.layouts.is_empty() {
            // An exhausted id space exports without the synthesized layout rather than with a
            // duplicated one (#407).
            let Some(raw) = q.next_id.max(1).checked_add(1) else {
                continue;
            };
            let id = LayoutId(raw);
            q.next_id = raw;
            Arc::make_mut(m).layouts.push(Layout {
                id,
                name: "Blank".into(),
                kind: joydeck_model::LayoutType::Blank,
                show_master_shapes: true,
                ..Default::default()
            });
        }
    }
    let first = q
        .masters
        .first()
        .and_then(|m| m.layouts.first())
        .map(|l| l.id)
        .unwrap_or_default();
    let ok: Vec<LayoutId> = q
        .masters
        .iter()
        .flat_map(|m| m.layouts.iter().map(|l| l.id))
        .collect();
    for s in q.slides.iter_mut() {
        if !ok.contains(&s.layout) {
            Arc::make_mut(s).layout = first;
        }
    }
    std::borrow::Cow::Owned(q)
}

pub fn export(p: &Presentation, main_ct: &str) -> Result<Vec<u8>, PptxError> {
    assemble(p, main_ct)?.finish()
}

pub(super) fn assemble(p: &Presentation, main_ct: &str) -> Result<PackageOut, PptxError> {
    let p = sanitized(p);
    let p: &Presentation = &p;
    let mut x = Exp {
        error: None,
        p,
        pkg: PackageOut::default(),
        media: Default::default(),
        n_chart: 0,
        n_diagram: 0,
        n_tags: 0,
        n_chartex: 0,
        n_kept: 0,
        slide_parts: HashMap::new(),
        table_styles: vec![],
    };
    for (i, s) in p.slides.iter().enumerate() {
        x.slide_parts
            .insert(s.id, format!("ppt/slides/slide{}.xml", i + 1));
    }
    let mut pres = Out::new("ppt/presentation.xml");

    let master_part::Written {
        ids: master_rids,
        layouts: layout_parts,
    } = master_part::write_graph(&mut x, &mut pres);

    // Notes master when a slide has notes, or the file already has one (its header and footer
    // live on the master, not on the notes slides).
    let notes_master = "ppt/notesMasters/notesMaster1.xml";
    let mut notes_master_rid = None;
    if let Some(nm) = notes_master_write(p) {
        let tname = format!("ppt/theme/theme{}.xml", p.masters.len() + 1);
        let mut no = Out::new(notes_master);
        let theme = nm
            .theme
            .or_else(|| p.masters.first().map(|m| m.theme.clone()))
            .unwrap_or_default();
        master_part::write_theme(&mut x, &mut no, &tname, &theme);
        let mut w = W::new();
        root_open(&mut w, "p:notesMaster", A::new());
        csld(
            &mut w,
            &mut x,
            &mut no,
            "",
            nm.background.as_ref(),
            &nm.shapes,
            &[],
        );
        master_part::write_mapping(&mut w, &nm.color_map);
        dml::list_style(&mut w, &mut x, &mut no, "p:notesStyle", &nm.style);
        w.close("p:notesMaster");
        x.pkg.xml_part(notes_master, CT_NOTES_MASTER, w.finish());
        x.pkg.rels(notes_master, &no.rels);
        notes_master_rid = Some(pres.rels.add(
            "notesMaster",
            &opc::relative("ppt/presentation.xml", notes_master),
            false,
        ));
    }

    // Handout master. Its header, footer, date and slide-number text live on this part; a deck
    // that has never had one does not gain an empty master on save.
    let handout_path = "ppt/handoutMasters/handoutMaster1.xml";
    let mut handout_rid = None;
    if let Some(hm) = p.handout_master.as_ref() {
        let tindex = p.masters.len() + 1 + usize::from(notes_master_rid.is_some());
        let tname = format!("ppt/theme/theme{tindex}.xml");
        let mut ho = Out::new(handout_path);
        master_part::write_theme(&mut x, &mut ho, &tname, &hm.theme);
        let mut w = W::new();
        root_open(&mut w, "p:handoutMaster", A::new());
        csld(
            &mut w,
            &mut x,
            &mut ho,
            &hm.name,
            hm.background.as_ref(),
            &hm.shapes,
            &[],
        );
        master_part::write_mapping(&mut w, &hm.color_map);
        w.close("p:handoutMaster");
        x.pkg.xml_part(handout_path, CT_HANDOUT_MASTER, w.finish());
        x.pkg.rels(handout_path, &ho.rels);
        handout_rid = Some(pres.rels.add(
            "handoutMaster",
            &opc::relative("ppt/presentation.xml", handout_path),
            false,
        ));
    }

    let mut review = crate::review_part::Writer::default();
    let mut n_comments = 0u32;

    // Slides.
    let mut slide_rids = vec![];
    for (i, s) in p.slides.iter().enumerate() {
        let sname = format!("ppt/slides/slide{}.xml", i + 1);
        let mut so = Out::new(&sname);
        if let Some(lp) = layout_parts.get(&s.layout) {
            so.rels
                .add("slideLayout", &opc::relative(&sname, lp), false);
        }
        if !s.notes.is_empty() {
            let nname = format!("ppt/notesSlides/notesSlide{}.xml", i + 1);
            so.rels
                .add("notesSlide", &opc::relative(&sname, &nname), false);
            let mut no = Out::new(&nname);
            no.rels
                .add("notesMaster", &opc::relative(&nname, notes_master), false);
            no.rels.add("slide", &opc::relative(&nname, &sname), false);
            let xml = crate::review_part::notes_xml(&mut x, &mut no, &s.notes);
            x.pkg.xml_part(&nname, CT_NOTES, xml);
            x.pkg.rels(&nname, &no.rels);
        }
        let mut comment_rel = None;
        if !s.comments.is_empty() {
            n_comments += 1;
            let cname = format!("ppt/comments/comment{n_comments}.xml");
            so.rels
                .add("comments", &opc::relative(&sname, &cname), false);
            x.pkg
                .xml_part(&cname, CT_COMMENTS, review.legacy(&s.comments));
            // The modern threaded part next to the flattened legacy one (issue #70):
            // PowerPoint 2021+ and DeckCraft read replies and resolved state from it,
            // older readers keep the legacy part.
            let mname = format!("ppt/comments/modernComment{n_comments}.xml");
            let rid = so
                .rels
                .add(RT_COMMENTS_MODERN, &opc::relative(&sname, &mname), false);
            x.pkg.xml_part(
                &mname,
                CT_COMMENTS_MODERN,
                review.modern(&s.comments, &cname),
            );
            comment_rel = Some(rid);
        }
        let xml = slide_xml(&mut x, &mut so, s, comment_rel.as_deref());
        x.pkg.xml_part(&sname, CT_SLIDE, xml);
        x.pkg.rels(&sname, &so.rels);
        slide_rids.push(pres.rels.add(
            "slide",
            &opc::relative("ppt/presentation.xml", &sname),
            false,
        ));
    }
    if let Some((legacy, modern)) = review.author_parts() {
        x.pkg.xml_part("ppt/commentAuthors.xml", CT_AUTHORS, legacy);
        pres.rels.add("commentAuthors", "commentAuthors.xml", false);
        x.pkg.xml_part("ppt/authors.xml", CT_AUTHORS_MODERN, modern);
        pres.rels.add(RT_AUTHORS_MODERN, "authors.xml", false);
    }

    // Presentation-level parts.
    let theme1 = "theme/theme1.xml";
    pres.rels.add("theme", theme1, false);
    pres.rels.add("presProps", "presProps.xml", false);
    pres.rels.add("viewProps", "viewProps.xml", false);
    pres.rels.add("tableStyles", "tableStyles.xml", false);
    x.pkg
        .xml_part("ppt/presProps.xml", CT_PRES_PROPS, pres_props_xml(p));
    x.pkg
        .xml_part("ppt/viewProps.xml", CT_VIEW_PROPS, view_props_xml(p));
    let ts = crate::tables::table_styles_xml(&x.table_styles, &p.table_styles);
    x.pkg.xml_part("ppt/tableStyles.xml", CT_TABLE_STYLES, ts);
    let pxml = presentation_xml(
        &mut x,
        &mut pres,
        &master_rids,
        notes_master_rid.as_deref(),
        handout_rid.as_deref(),
        &slide_rids,
    );
    x.pkg.xml_part("ppt/presentation.xml", main_ct, pxml);
    // Kept parts never replace a part the writer makes.
    let kept: Vec<_> = p
        .kept_parts
        .iter()
        .filter(|k| {
            let n = k.name.to_ascii_lowercase();
            !n.is_empty()
                && !n.contains("..")
                && !n.starts_with("ppt/")
                && !n.starts_with("docprops/core")
                && !n.starts_with("docprops/app")
                && !n.starts_with("[content_types]")
                && n != "_rels/.rels"
        })
        .collect();
    for k in kept
        .iter()
        .filter(|k| !k.rel_type.is_empty() && !k.from_root)
    {
        pres.rels.add_new(
            &k.rel_type,
            &opc::relative("ppt/presentation.xml", &k.name),
            false,
        );
    }
    x.pkg.rels("ppt/presentation.xml", &pres.rels);
    for k in &kept {
        if k.content_type.is_empty() {
            x.pkg.raw(&k.name, k.xml.clone().into_bytes());
        } else {
            x.pkg
                .xml_part(&k.name, &k.content_type, k.xml.clone().into_bytes());
        }
    }

    // Document properties and package relationships.
    x.pkg.xml_part("docProps/core.xml", CT_CORE, core_xml(p));
    x.pkg.xml_part("docProps/app.xml", CT_APP, app_xml(p));
    let mut root = RelsOut::default();
    root.add("officeDocument", "ppt/presentation.xml", false);
    root.add(opc::RT_CORE, "docProps/core.xml", false);
    root.add("extended-properties", "docProps/app.xml", false);
    for k in kept
        .iter()
        .filter(|k| !k.rel_type.is_empty() && k.from_root)
    {
        root.add_new(&k.rel_type, &k.name, false);
    }
    x.pkg.rels("", &root);
    if let Some(error) = x.error {
        return Err(error);
    }
    Ok(x.pkg)
}

fn slide_xml(x: &mut Exp, o: &mut Out, s: &Slide, comment_rel: Option<&str>) -> Vec<u8> {
    let mut w = W::new();
    root_open(
        &mut w,
        "p:sld",
        A::new()
            .o("showMasterSp", (!s.show_master_shapes).then_some("0"))
            .o("show", s.hidden.then_some("0")),
    );
    let ids = csld(
        &mut w,
        x,
        o,
        &s.name,
        s.background.as_ref(),
        &s.shapes,
        &s.tags,
    );
    master_part::write_override(&mut w, s.color_map.as_deref());
    if let Some(t) = &s.transition {
        crate::transition_part::write(&mut w, x, o, t);
    }
    if !s.animations.is_empty() {
        timing::timing(&mut w, x, &s.animations, &s.shapes, &ids);
    }
    slide_ext_list(&mut w, s.raw_ext.as_deref(), comment_rel);
    w.close("p:sld");
    w.finish()
}

/// Sections with every slide in exactly one section, in slide order.
fn section_slides(p: &Presentation) -> Vec<(String, Vec<usize>)> {
    let mut out: Vec<(String, Vec<usize>)> = p
        .sections
        .iter()
        .map(|s| (s.name.clone(), vec![]))
        .collect();
    if out.is_empty() {
        return out;
    }
    let mut cur = 0usize;
    for (i, s) in p.slides.iter().enumerate() {
        if let Some(k) = p.sections.iter().position(|sec| sec.slides.contains(&s.id))
            && k >= cur
        {
            cur = k;
        }
        if let Some(sec) = out.get_mut(cur) {
            sec.1.push(i);
        }
    }
    out
}

/// One embedded typeface, with the relationship id of each face that had bytes.
struct WrittenFont {
    typeface: String,
    panose: Option<String>,
    pitch_family: Option<u8>,
    charset: Option<u8>,
    regular: Option<String>,
    bold: Option<String>,
    italic: Option<String>,
    bold_italic: Option<String>,
}

impl WrittenFont {
    fn any(&self) -> bool {
        self.regular.is_some()
            || self.bold.is_some()
            || self.italic.is_some()
            || self.bold_italic.is_some()
    }
}

/// Font parts for `p:embeddedFontLst`. One part per distinct media item, named `ppt/fonts/fontN.fntdata`.
fn write_embedded_fonts(x: &mut Exp, o: &mut Out) -> Vec<WrittenFont> {
    struct Plan {
        typeface: String,
        panose: Option<String>,
        pitch_family: Option<u8>,
        charset: Option<u8>,
        regular: Option<MediaId>,
        bold: Option<MediaId>,
        italic: Option<MediaId>,
        bold_italic: Option<MediaId>,
    }
    let mut plans = Vec::new();
    for font in x.p.embedded_fonts.iter().take(64) {
        let typeface = font.typeface.trim();
        if typeface.is_empty() {
            continue;
        }
        plans.push(Plan {
            typeface: typeface.chars().take(255).collect(),
            panose: font
                .panose
                .clone()
                .filter(|s| s.len() == 20 && s.chars().all(|c| c.is_ascii_hexdigit())),
            pitch_family: font.pitch_family,
            charset: font.charset,
            regular: font.regular,
            bold: font.bold,
            italic: font.italic,
            bold_italic: font.bold_italic,
        });
    }
    let mut seen_part: HashMap<MediaId, String> = HashMap::new();
    let mut n = 0u32;
    let mut rel = |x: &mut Exp, o: &mut Out, id: Option<MediaId>| -> Option<String> {
        let id = id?;
        if let Some(rid) = seen_part.get(&id) {
            return Some(rid.clone());
        }
        let m = x.p.media(id)?;
        if m.data.is_empty() || m.data.len() > 32 * 1024 * 1024 {
            return None;
        }
        let bytes = m.data.as_ref().clone();
        n = n.saturating_add(1);
        let name = format!("ppt/fonts/font{n}.fntdata");
        x.pkg.binary(&name, "application/x-fontdata", bytes);
        let rid = o
            .rels
            .add("font", &opc::relative("ppt/presentation.xml", &name), false);
        seen_part.insert(id, rid.clone());
        Some(rid)
    };
    let mut seen_face = Vec::new();
    let mut out = Vec::new();
    for font in plans {
        let written = WrittenFont {
            typeface: font.typeface,
            panose: font.panose,
            pitch_family: font.pitch_family,
            charset: font.charset,
            regular: rel(x, o, font.regular),
            bold: rel(x, o, font.bold),
            italic: rel(x, o, font.italic),
            bold_italic: rel(x, o, font.bold_italic),
        };
        if written.any() && !seen_face.iter().any(|s| s == &written.typeface) {
            seen_face.push(written.typeface.clone());
            out.push(written);
        }
    }
    out
}

fn presentation_xml(
    x: &mut Exp,
    o: &mut Out,
    masters: &[(u64, String)],
    notes: Option<&str>,
    handout: Option<&str>,
    slides: &[String],
) -> Vec<u8> {
    let fonts = write_embedded_fonts(x, o);
    let p = x.p;
    let mut w = W::new();
    let first = (p.first_slide_number != 1).then_some(p.first_slide_number.min(9999));
    root_open(
        &mut w,
        "p:presentation",
        A::new()
            .a("saveSubsetFonts", 1)
            .t("embedTrueTypeFonts", !fonts.is_empty())
            .o("firstSlideNum", first),
    );
    w.open0("p:sldMasterIdLst");
    for (id, rid) in masters {
        w.empty("p:sldMasterId", A::new().a("id", id).a("r:id", rid));
    }
    w.close("p:sldMasterIdLst");
    if let Some(n) = notes {
        w.open0("p:notesMasterIdLst");
        w.empty("p:notesMasterId", A::new().a("r:id", n));
        w.close("p:notesMasterIdLst");
    }
    if let Some(h) = handout {
        w.open0("p:handoutMasterIdLst");
        w.empty("p:handoutMasterId", A::new().a("r:id", h));
        w.close("p:handoutMasterIdLst");
    }
    if !slides.is_empty() {
        w.open0("p:sldIdLst");
        for (i, rid) in slides.iter().enumerate() {
            w.empty("p:sldId", A::new().a("id", 256 + i).a("r:id", rid));
        }
        w.close("p:sldIdLst");
    }
    let clamp = |v: f64| dml::emu(v).clamp(914_400, 51_206_400);
    w.empty(
        "p:sldSz",
        A::new()
            .a("cx", clamp(p.slide_size.width))
            .a("cy", clamp(p.slide_size.height)),
    );
    w.empty(
        "p:notesSz",
        A::new()
            .a("cx", dml::emu_pos(p.notes_size.width).max(1))
            .a("cy", dml::emu_pos(p.notes_size.height).max(1)),
    );
    if !fonts.is_empty() {
        w.open0("p:embeddedFontLst");
        for f in &fonts {
            w.open0("p:embeddedFont");
            w.empty(
                "p:font",
                A::new()
                    .a("typeface", &f.typeface)
                    .o("panose", f.panose.clone())
                    .o("pitchFamily", f.pitch_family)
                    .o("charset", f.charset),
            );
            for (tag, rid) in [
                ("p:regular", &f.regular),
                ("p:bold", &f.bold),
                ("p:italic", &f.italic),
                ("p:boldItalic", &f.bold_italic),
            ] {
                if let Some(rid) = rid {
                    w.empty(tag, A::new().a("r:id", rid));
                }
            }
            w.close("p:embeddedFont");
        }
        w.close("p:embeddedFontLst");
    }
    if !p.custom_shows.is_empty() {
        w.open0("p:custShowLst");
        for (i, cs) in p.custom_shows.iter().enumerate() {
            w.open("p:custShow", A::new().a("name", &cs.name).a("id", i));
            w.open0("p:sldLst");
            for sid in &cs.slides {
                if let Some(part) = x.slide_parts.get(sid).cloned() {
                    let rid = o.rels.add(
                        "slide",
                        &opc::relative("ppt/presentation.xml", &part),
                        false,
                    );
                    w.empty("p:sld", A::new().a("r:id", rid));
                }
            }
            w.close("p:sldLst");
            w.close("p:custShow");
        }
        w.close("p:custShowLst");
    }
    cust_data(&mut w, x, o, &p.tags);
    let dts = if p.default_text_style.is_empty() {
        defaults::default_text_style()
    } else {
        p.default_text_style.clone()
    };
    dml::list_style(&mut w, x, o, "p:defaultTextStyle", &dts);
    let sections = section_slides(p);
    let raw = p.raw_ext.as_deref().filter(|r| !r.trim().is_empty());
    if !sections.is_empty() || raw.is_some() {
        let mut inner = W::frag();
        if !sections.is_empty() {
            inner.open("p:ext", A::new().a("uri", crate::SECTION_EXT_URI));
            inner.open("p14:sectionLst", A::new().a("xmlns:p14", NS_P14));
            for (i, (name, list)) in sections.iter().enumerate() {
                inner.open(
                    "p14:section",
                    A::new()
                        .a("name", name)
                        .a("id", crate::tables::guid(&format!("section:{i}:{name}"))),
                );
                inner.open0("p14:sldIdLst");
                for si in list {
                    inner.empty("p14:sldId", A::new().a("id", 256 + si));
                }
                inner.close("p14:sldIdLst");
                inner.close("p14:section");
            }
            inner.close("p14:sectionLst");
            inner.close("p:ext");
        }
        if let Some(r) = raw {
            inner.raw(r);
        }
        ext_list(&mut w, Some(&inner.s));
    }
    w.close("p:presentation");
    w.finish()
}

fn pres_props_xml(p: &Presentation) -> Vec<u8> {
    let s = &p.show;
    let mut w = W::new();
    root_open(&mut w, "p:presentationPr", A::new());
    w.open(
        "p:showPr",
        A::new()
            .t("loop", s.loop_until_esc)
            .a("showNarration", if s.without_narration { 0 } else { 1 })
            .o("showAnimation", s.without_animation.then_some("0"))
            .o("useTimings", (!s.use_timings).then_some("0")),
    );
    match s.show_type.as_str() {
        "browsed" => w.empty("p:browse", A::new()),
        "kiosk" => w.empty("p:kiosk", A::new()),
        _ => w.empty0("p:present"),
    }
    let custom = s
        .custom_show
        .as_ref()
        .and_then(|n| p.custom_shows.iter().position(|c| c.name == *n));
    match (custom, s.range) {
        (Some(i), _) => w.empty("p:custShow", A::new().a("id", i)),
        (None, Some((a, b))) => w.empty(
            "p:sldRg",
            A::new().a("st", a.max(1)).a("end", b.max(a).max(1)),
        ),
        _ => w.empty0("p:sldAll"),
    }
    w.open0("p:penClr");
    w.val("a:srgbClr", s.pen_color.hex());
    w.close("p:penClr");
    w.close("p:showPr");
    w.close("p:presentationPr");
    w.finish()
}

fn view_props_xml(p: &Presentation) -> Vec<u8> {
    let mut w = W::new();
    root_open(&mut w, "p:viewPr", A::new());
    w.open0("p:normalViewPr");
    w.empty("p:restoredLeft", A::new().a("sz", 15620));
    w.empty("p:restoredTop", A::new().a("sz", 94660));
    w.close("p:normalViewPr");
    if !p.guides.is_empty() {
        // Drawing guides; `pos` is in eighths of a point (576 per inch).
        w.open0("p:slideViewPr");
        w.open0("p:cSldViewPr");
        w.open0("p:cViewPr");
        w.open0("p:scale");
        w.empty("a:sx", A::new().a("n", 100).a("d", 100));
        w.empty("a:sy", A::new().a("n", 100).a("d", 100));
        w.close("p:scale");
        w.empty("p:origin", A::new().a("x", 0).a("y", 0));
        w.close("p:cViewPr");
        w.open0("p:guideLst");
        for g in p.guides.iter().take(1000) {
            let pos = (g.pos * 8.0).round().clamp(-1.0e9, 1.0e9) as i64;
            let a = if g.vertical {
                A::new().a("pos", pos)
            } else {
                A::new().a("orient", "horz").a("pos", pos)
            };
            w.empty("p:guide", a);
        }
        w.close("p:guideLst");
        w.close("p:cSldViewPr");
        w.close("p:slideViewPr");
    }
    w.empty("p:gridSpacing", A::new().a("cx", 76200).a("cy", 76200));
    w.close("p:viewPr");
    w.finish()
}

fn core_xml(p: &Presentation) -> Vec<u8> {
    let pr = &p.props;
    let mut w = W::new();
    w.open(
        "cp:coreProperties",
        A::new()
            .a(
                "xmlns:cp",
                "http://schemas.openxmlformats.org/package/2006/metadata/core-properties",
            )
            .a("xmlns:dc", "http://purl.org/dc/elements/1.1/")
            .a("xmlns:dcterms", "http://purl.org/dc/terms/")
            .a("xmlns:dcmitype", "http://purl.org/dc/dcmitype/")
            .a("xmlns:xsi", "http://www.w3.org/2001/XMLSchema-instance"),
    );
    for (tag, v) in [
        ("dc:title", &pr.title),
        ("dc:subject", &pr.subject),
        ("dc:creator", &pr.author),
        ("cp:keywords", &pr.keywords),
        ("dc:description", &pr.comments),
        ("cp:category", &pr.category),
        ("cp:lastModifiedBy", &pr.last_modified_by),
    ] {
        if !v.is_empty() {
            w.elt(tag, v);
        }
    }
    if pr.revision > 0 {
        w.elt("cp:revision", &pr.revision.to_string());
    }
    for (tag, v) in [
        ("dcterms:created", &pr.created),
        ("dcterms:modified", &pr.modified),
    ] {
        if valid_date(v) {
            w.open(tag, A::new().a("xsi:type", "dcterms:W3CDTF"));
            w.text(v);
            w.close(tag);
        }
    }
    w.close("cp:coreProperties");
    w.finish()
}

fn app_xml(p: &Presentation) -> Vec<u8> {
    let mut w = W::new();
    w.open(
        "Properties",
        A::new()
            .a(
                "xmlns",
                "http://schemas.openxmlformats.org/officeDocument/2006/extended-properties",
            )
            .a(
                "xmlns:vt",
                "http://schemas.openxmlformats.org/officeDocument/2006/docPropsVTypes",
            ),
    );
    w.elt("Application", "DeckCraft");
    w.elt("Slides", &p.slides.len().to_string());
    w.elt(
        "Notes",
        &p.slides
            .iter()
            .filter(|s| !s.notes.is_empty())
            .count()
            .to_string(),
    );
    w.elt(
        "HiddenSlides",
        &p.slides.iter().filter(|s| s.hidden).count().to_string(),
    );
    if !p.props.company.is_empty() {
        w.elt("Company", &p.props.company);
    }
    w.close("Properties");
    w.finish()
}
