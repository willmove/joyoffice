extern crate joydeck_shell as joydeck;
use joydeck::{
    authoring_demo, chart,
    design_commands::{Action, PageTransfer},
    diagram,
    edit::{self, Command},
    export, interop,
    motion::{self, Controller, Effect, Timing},
    pptx_write, preview, scene,
    show::ShowSnapshot,
};
use std::sync::{Arc, atomic::AtomicBool};
fn cancel() -> AtomicBool {
    AtomicBool::new(false)
}
fn public(d: &joydeck::document::Deck) -> joydeck::document::Deck {
    let mut p = pptx_write::generate(d).unwrap();
    p.get_mut("ppt/slides/slide1.xml")
        .unwrap()
        .extend_from_slice(b"<!-- public -->");
    let path = std::env::temp_dir().join(format!(
        "joydeck-public-{}-{}.pptx",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    export::save_parts(&path, &p, &cancel()).unwrap();
    let d = interop::import(&path).unwrap();
    std::fs::remove_file(path).unwrap();
    d
}
#[test]
fn chart_empty_negative_scatter_and_atomic_paste() {
    let mut c = chart::Chart::default();
    c.paste("分类\t一\t二\nA\t-4\t\nB\t2\t8").unwrap();
    assert_eq!(c.series[1].values, vec![None, Some(8.)]);
    assert_eq!(c.domain(), (-4., 8.));
    let old = c.clone();
    for invalid in ["a\tb\nA\tNaN", "a\tb\nA\t1\t2", "a\tb\nA\tinf"] {
        assert!(c.paste(invalid).is_err());
        assert_eq!(c, old);
    }
    c.kind = "scatter".into();
    c.paste("X1\tY1\tX2\tY2\n1\t2\t10\t4\n\t5\t20\t8").unwrap();
    let text = c.tsv();
    let original = c.clone();
    c.paste(&text).unwrap();
    assert_eq!(c, original);
    assert_eq!(c.series[0].x[1], None);
}
#[test]
fn chart_all_types_public_cache_workbook_and_holes() {
    let mut d = authoring_demo::deck();
    d.slides[0].objects[1].chart.as_mut().unwrap().series[0].values[1] = None;
    let p = pptx_write::generate(&d).unwrap();
    let xlsx: Vec<_> = p.iter().filter(|(n, _)| n.ends_with(".xlsx")).collect();
    assert_eq!(xlsx.len(), 7);
    for (_, bytes) in xlsx {
        let mut z = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
        let mut s = String::new();
        std::io::Read::read_to_string(&mut z.by_name("xl/worksheets/sheet1.xml").unwrap(), &mut s)
            .unwrap();
        assert!(s.contains("<v>91</v>"));
    }
    let r = public(&d);
    for i in [0, 1, 2, 3, 4, 7, 8] {
        let a = d.slides[i].objects[1].chart.as_ref().unwrap();
        let b = r.slides[i]
            .objects
            .iter()
            .find_map(|o| o.chart.as_ref())
            .unwrap();
        assert_eq!(a.series, b.series);
        assert_eq!(a.kind, b.kind);
        assert_eq!(a.x_title, b.x_title);
    }
    assert_eq!(
        r.slides[0]
            .objects
            .iter()
            .find_map(|o| o.chart.as_ref())
            .unwrap()
            .series[0]
            .values[1],
        None
    );
}
#[test]
fn chart_validation_aggregate_and_palette() {
    let mut d = authoring_demo::deck();
    let c = d.slides[0].objects[1].chart.as_mut().unwrap();
    c.maximum = Some(f64::INFINITY);
    assert!(d.validate().is_err());
    let c = d.slides[0].objects[1].chart.as_mut().unwrap();
    c.maximum = Some(-1.);
    c.minimum = Some(3.);
    assert!(d.validate().is_err());
    let mut c = chart::Chart {
        kind: "pie".into(),
        ..Default::default()
    };
    c.series[0].values[0] = Some(-1.);
    assert!(c.validate().is_err());
}
#[test]
fn chart_and_diagram_native_scene_and_pdf() {
    let d = authoring_demo::deck();
    for page in 0..9 {
        let a = preview::render(&d, page, 960, &cancel()).unwrap();
        assert!(
            a.bgra
                .as_chunks::<4>()
                .0
                .iter()
                .filter(|p| p[0] < 200 && p[1] < 200)
                .count()
                > 100
        );
        let mut cache = scene::SceneCache::default();
        let s = cache.build(&d, page, 1, 1, &cancel()).unwrap();
        assert!(!s.nodes.is_empty());
    }
    let dir = std::env::temp_dir().join(format!("joydeck-m9-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    edit::save(&dir.join("test.joydeck"), &d).unwrap();
    assert_eq!(edit::open_editable(&dir.join("test.joydeck")).unwrap(), d);
    let pdf = dir.join("chart.pdf");
    let _ = std::fs::remove_file(&pdf);
    export::pdf(
        &pdf,
        &d,
        &export::Options {
            pages: vec![0, 5],
            dpi: 72,
            notes: false,
            transparent: false,
        },
        &cancel(),
    )
    .unwrap();
    assert!(std::fs::metadata(pdf).unwrap().len() > 1000);
}
#[test]
fn area_gaps_and_doughnut_hole_survive_public_formats() {
    let mut d = (*edit::Session::new().deck).clone();
    let mut o = chart::object("area");
    let c = o.chart.as_mut().unwrap();
    c.paste("分类\t数值\nA\t2\nB\t\nC\t-1\nD\t3").unwrap();
    c.labels = false;
    let body = chart::svg(&d, 0, &o).unwrap();
    let xml = format!("<svg xmlns=\"http://www.w3.org/2000/svg\">{body}</svg>");
    let doc = roxmltree::Document::parse(&xml).unwrap();
    let area = doc
        .descendants()
        .find(|n| n.has_tag_name("path") && n.attribute("fill") == Some("#4472c4"))
        .unwrap();
    assert_eq!(
        area.attribute("d").unwrap().matches('Z').count(),
        2,
        "missing value must split area rather than bridge it"
    );
    d.slides[0].objects = vec![o];
    let r = public(&d);
    assert_eq!(
        r.slides[0].objects[0].chart.as_ref().unwrap().series[0].values,
        vec![Some(2.), None, Some(-1.), Some(3.)]
    );
    let mut o = chart::object("doughnut");
    let c = o.chart.as_mut().unwrap();
    c.paste("分类\t数值\nA\t1").unwrap();
    c.labels = false;
    d.slides[0].objects = vec![o];
    let r = public(&d);
    assert_eq!(
        r.slides[0].objects[0].chart.as_ref().unwrap().kind,
        "doughnut"
    );
    let ring = preview::render(&d, 0, 960, &cancel()).unwrap();
    d.slides[0].objects[0].chart.as_mut().unwrap().kind = "pie".into();
    let pie = preview::render(&d, 0, 960, &cancel()).unwrap();
    let p = (267 * 960 + 487) * 4;
    assert!(
        ring.bgra[p..p + 3].iter().all(|v| *v > 245),
        "ring hole must expose the background"
    );
    assert!(
        pie.bgra[p..p + 3].iter().any(|v| *v < 200),
        "single-slice pie must remain filled"
    );
}
#[test]
fn diagram_tree_switch_and_atomic_cycle_rejection() {
    let mut d = diagram::Diagram::default();
    d.set_text("A\n\tB\n\t\tC\nD").unwrap();
    assert_eq!(d.text(), "A\n\tB\n\t\tC\nD");
    let old = d.clone();
    assert!(d.set_text("\t\tinvalid").is_err());
    assert_eq!(d, old);
    d.nodes[0].parent = Some(d.nodes[2].id.clone());
    assert!(d.validate().is_err());
    let mut o = diagram::object("hierarchy");
    let mut old = old;
    old.layout = "hierarchy".into();
    o.diagram = Some(old);
    assert_eq!(
        diagram::expand(&o)
            .unwrap()
            .iter()
            .filter(|o| o.kind == "line")
            .count(),
        2
    );
}
#[test]
fn chart_data_and_diagram_conversion_are_one_undo() {
    let mut s = edit::Session::from_deck(authoring_demo::deck(), false);
    let base = s.deck.clone();
    let mut o = s.deck.slides[0].objects[1].clone();
    o.chart.as_mut().unwrap().series[0].values[0] = Some(131.);
    s.execute(s.id, Command::ReplaceObject(0, 1, o)).unwrap();
    s.undo();
    assert_eq!(s.deck, base);
    let mut cmds = vec![Command::DeleteObject(5, 1)];
    cmds.extend(
        diagram::expand(&s.deck.slides[5].objects[1])
            .unwrap()
            .into_iter()
            .map(|mut o| {
                o.id.clear();
                Command::AddObject(5, o)
            }),
    );
    s.execute(s.id, Command::Batch(cmds)).unwrap();
    s.undo();
    assert_eq!(s.deck, base);
}
#[test]
fn motion_click_with_after_repeat_reverse_rewind_and_seek() {
    let mut d = authoring_demo::deck();
    let id = d.slides[0].id.clone();
    let target = d.slides[0].objects[1].id.clone();
    let e = Effect {
        target: target.clone(),
        duration_ms: 100,
        delay_ms: 20,
        repeats: 2,
        auto_reverse: true,
        ..Default::default()
    };
    d.presentation.slides.insert(
        id,
        Timing {
            effects: vec![e.clone()],
            ..Default::default()
        },
    );
    let sample = |step, ms| motion::sample(&d, 0, step, ms).unwrap().slides[0].objects[1].opacity;
    assert_eq!(sample(0, 999), 0.);
    assert_eq!(sample(1, 10), 0.);
    assert!((sample(1, 70) - 0.5).abs() < 1e-6);
    assert!((sample(1, 170) - 0.5).abs() < 1e-6);
    assert_eq!(sample(1, 420), 0.);
    assert_eq!(sample(1, 20), 0.);
    let mut t = Timing {
        effects: vec![
            e.clone(),
            Effect {
                id: "with".into(),
                trigger: "with".into(),
                delay_ms: 10,
                ..e.clone()
            },
            Effect {
                id: "after".into(),
                trigger: "after".into(),
                delay_ms: 30,
                ..e
            },
        ],
        ..Default::default()
    };
    let times = motion::schedule(&t);
    assert_eq!(
        (times[1].step, times[1].start, times[2].start),
        (1, 30, 460)
    );
    t.effects[0].rewind = true;
}
#[test]
fn controller_snapshot_pause_custom_order_loop_revisit() {
    let mut d = authoring_demo::deck();
    d.presentation.order = vec![d.slides[2].id.clone(), d.slides[0].id.clone()];
    d.presentation.loop_show = true;
    let mut s = ShowSnapshot::new(Arc::new(d), 0).unwrap();
    assert_eq!(s.page(), 0);
    let mut c = Controller {
        paused: true,
        ..Default::default()
    };
    c.advance(&mut s, 100);
    assert_eq!(c.total, 0);
    c.paused = false;
    c.next(&mut s);
    assert_eq!(c.step, 1);
    c.advance(&mut s, 300);
    c.next(&mut s);
    assert_eq!(s.page(), 0);
    assert_eq!(c.elapsed, 600);
    c.next(&mut s);
    assert_eq!(s.page(), 2);
    assert_eq!(c.step, 0);
    c.previous(&mut s);
    assert_eq!(c.step, 1);
    c.seek(123);
    assert_eq!(c.elapsed, 123);
}
#[test]
fn motion_invalid_ids_budget_and_reconcile_undo() {
    let mut s = edit::Session::from_deck(authoring_demo::deck(), false);
    let base = s.deck.clone();
    let mut p = s.deck.presentation.clone();
    p.slides.values_mut().next().unwrap().effects[0].duration_ms = 0;
    assert!(s.execute(s.id, Command::Motion(p)).is_err());
    assert_eq!(s.deck, base);
    s.execute(s.id, Command::DeleteObject(0, 1)).unwrap();
    assert!(s.deck.presentation.slides["page-0"].effects.is_empty());
    s.undo();
    assert_eq!(s.deck, base);
    let mut d = (*base).clone();
    d.slides[0].objects[0].morph_key = Some("same".into());
    d.slides[0].objects[1].morph_key = Some("same".into());
    d.presentation.slides.clear();
    assert!(d.validate().is_err());
}
#[test]
fn morph_geometry_unmatched_opacity_and_crop() {
    let mut d = authoring_demo::deck();
    d.slides[0].objects[0].morph_key = Some("title".into());
    d.slides[1].objects[0].morph_key = Some("title".into());
    d.slides[1].objects[0].x = 154.;
    d.slides[1].objects[0].text = d.slides[0].objects[0].text.clone();
    let f = motion::morph(&d, 0, 1, 0.5).unwrap();
    let o = f.slides[1]
        .objects
        .iter()
        .find(|o| o.id == "title-1")
        .unwrap();
    assert_eq!(o.x, 104.);
    let current = f.slides[1]
        .objects
        .iter()
        .find(|o| o.id == "content-1")
        .unwrap();
    assert_eq!(current.opacity, 0.5);
    assert!(
        f.slides[1]
            .objects
            .iter()
            .any(|o| o.id.starts_with("morph-out:"))
    );
}
#[test]
fn paragraph_animation_preserves_byte_runs_and_document() {
    let mut d = authoring_demo::deck();
    let o = &mut d.slides[0].objects[0];
    o.text = "中文\nSecond".into();
    let id = o.id.clone();
    let t = d.presentation.slides.get_mut("page-0").unwrap();
    t.effects = vec![Effect {
        target: id,
        paragraph: Some(1),
        ..Default::default()
    }];
    d.validate().unwrap();
    let before = d.clone();
    let f = motion::sample(&d, 0, 0, 1000).unwrap();
    assert_eq!(f.slides[0].objects[0].text, "中文");
    assert_eq!(f.slides[0].objects[1].opacity, 0.);
    assert_eq!(d, before);
}
#[test]
fn public_animation_fade_and_actual_powerpoint_sample() {
    let d = authoring_demo::deck();
    let p = public(&d);
    let t = p
        .presentation
        .slides
        .values()
        .find(|t| !t.effects.is_empty())
        .unwrap();
    assert_eq!(
        (t.effects[0].kind.as_str(), t.effects[0].duration_ms),
        ("fade", 600)
    );
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/joydeck/m10-powerpoint-authored-motion.pptx");
    let imported = interop::import(&path).unwrap();
    let t = imported
        .presentation
        .slides
        .values()
        .find(|t| !t.effects.is_empty())
        .unwrap();
    assert_eq!(
        (t.effects[0].kind.as_str(), t.effects[0].duration_ms),
        ("fade", 500)
    );
    let native =
        std::env::temp_dir().join(format!("joydeck-m10-pp-{}.joydeck", std::process::id()));
    edit::save(&native, &imported).unwrap();
    let reopened = edit::open_editable(&native).unwrap();
    assert_eq!(reopened.presentation, imported.presentation);
    assert_eq!(reopened.slides, imported.slides);
    assert!(reopened.source.as_ref().unwrap().parts == imported.source.as_ref().unwrap().parts);
}
#[test]
fn transition_pixels_boundaries_fade_push_wipe() {
    let a = preview::Raster {
        width: 2,
        height: 1,
        bgra: vec![0; 8],
    };
    for kind in ["fade", "push", "wipe"] {
        for p in [0., 1.] {
            let mut b = preview::Raster {
                width: 2,
                height: 1,
                bgra: vec![200; 8],
            };
            motion::composite(&a, &mut b, kind, p).unwrap();
            assert_eq!(b.bgra, vec![if p == 0. { 0 } else { 200 }; 8]);
        }
    }
}
#[test]
fn page_copy_and_cross_document_transfer_remap_effects() {
    let mut s = edit::Session::from_deck(authoring_demo::deck(), false);
    s.execute(s.id, Command::DuplicatePage(0)).unwrap();
    let copied = s.deck.slides.last().unwrap();
    let e = &s.deck.presentation.slides[&copied.id].effects[0];
    assert!(copied.objects.iter().any(|o| o.id == e.target));
    let t = PageTransfer::capture(&s.deck, &[0]).unwrap();
    let mut dest = edit::Session::new();
    dest.execute(dest.id, Command::Design(Action::PastePages(1, t, None)))
        .unwrap();
    let copied = &dest.deck.slides[1];
    assert!(
        dest.deck.presentation.slides[&copied.id]
            .effects
            .iter()
            .all(|e| copied.objects.iter().any(|o| o.id == e.target))
    );
}
#[test]
fn ink_budget_and_annotation_copy() {
    let d = authoring_demo::deck();
    let mut copy = d.clone();
    motion::append_ink(
        &mut copy,
        &[motion::Ink {
            page: 0,
            points: vec![(20., 20.), (100., 80.), (150., 30.)],
        }],
    )
    .unwrap();
    copy.validate().unwrap();
    assert_eq!(copy.slides[0].objects.len(), d.slides[0].objects.len() + 2);
    assert!(
        motion::append_ink(
            &mut copy,
            &[motion::Ink {
                page: 0,
                points: vec![(f32::NAN, 1.), (2., 3.)]
            }]
        )
        .is_err()
    );
}
#[test]
fn actual_powerpoint_chart_edit_and_stale_workbook_detection() {
    let fixtures =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/joydeck");
    let edited = interop::import(&fixtures.join("m9-powerpoint-edited-chart.pptx")).unwrap();
    let chart = edited.slides[0]
        .objects
        .iter()
        .find_map(|o| o.chart.as_ref())
        .expect("actual PowerPoint chart must import");
    assert_eq!(chart.series[0].values[3], Some(133.));
    assert!(edited.presentation.slides.values().any(|t| {
        t.effects
            .iter()
            .any(|e| e.kind == "fade" && e.duration_ms == 600)
    }));
    let stale = interop::import(&fixtures.join("m9-powerpoint-stale-cache.pptx")).unwrap();
    assert!(
        stale
            .warnings
            .iter()
            .any(|w| w.message.contains("缓存与嵌入工作簿"))
    );
    assert!(!stale.slides[0].objects.iter().any(|o| o.chart.is_some()));
    assert!(stale.source.as_ref().unwrap().preserve(&stale).is_ok());
}
#[test]
fn links_rehearsal_and_automatic_advance_share_controller() {
    let mut d = authoring_demo::deck();
    d.presentation
        .links
        .insert("title-0".into(), "page-2".into());
    let mut s = ShowSnapshot::new(Arc::new(d), 0).unwrap();
    let mut c = Controller {
        rehearsing: true,
        ..Default::default()
    };
    c.advance(&mut s, 2000);
    c.next_page(&mut s);
    assert_eq!(c.rehearsed["page-0"], 2000);
    c.follow_link(&mut s, "title-0").unwrap();
    assert_eq!(s.page(), 2);
    let mut d = (*s.deck).clone();
    d.presentation.slides.get_mut("page-2").unwrap().advance_ms = Some(200);
    s.deck = Arc::new(d);
    c.advance(&mut s, 201);
    assert_eq!(s.page(), 3);
}
#[test]
fn unknown_chart_smartart_ole_closure_survives_native_and_preserve() {
    let d = authoring_demo::deck();
    let mut parts = pptx_write::generate(&d).unwrap();
    let types = String::from_utf8(parts["[Content_Types].xml"].clone())
        .unwrap()
        .replace(
            "</Types>",
            "<Default Extension=\"bin\" ContentType=\"application/octet-stream\"/></Types>",
        );
    parts.insert("[Content_Types].xml".into(), types.into_bytes());
    let rel = "ppt/charts/_rels/chart-1-3.xml.rels";
    let mut text = String::from_utf8(parts[rel].clone()).unwrap();
    text=text.replace("</Relationships>",r#"<Relationship Id="future" Type="urn:joydeck:test:chartex" Target="../diagrams/unknown.xml"/></Relationships>"#);
    parts.insert(rel.into(), text.into_bytes());
    parts.insert(
        "ppt/diagrams/unknown.xml".into(),
        b"<opaque xmlns=\"urn:original-test\">SmartArt/chartex</opaque>".to_vec(),
    );
    parts.insert("ppt/diagrams/_rels/unknown.xml.rels".into(),br#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="ole" Type="urn:joydeck:test:ole" Target="../embeddings/opaque.bin"/></Relationships>"#.to_vec());
    parts.insert("ppt/embeddings/opaque.bin".into(), vec![0, 1, 2, 255]);
    let path = std::env::temp_dir().join(format!("joydeck-unknown-m9-{}.pptx", std::process::id()));
    let _ = std::fs::remove_file(&path);
    export::save_parts(&path, &parts, &cancel()).unwrap();
    let imported = interop::import(&path).unwrap();
    let original = imported
        .source
        .as_ref()
        .unwrap()
        .preserve(&imported)
        .unwrap();
    assert_eq!(original["ppt/embeddings/opaque.bin"], vec![0, 1, 2, 255]);
    let mut changed = imported.clone();
    changed.slides[0]
        .objects
        .iter_mut()
        .find_map(|o| o.chart.as_mut())
        .unwrap()
        .series[0]
        .values[0] = Some(133.);
    assert!(changed.source.as_ref().unwrap().preserve(&changed).is_err());
    let file = path.with_extension("joydeck");
    edit::save(&file, &changed).unwrap();
    let reopened = edit::open_editable(&file).unwrap();
    assert_eq!(
        reopened.source.as_ref().unwrap().parts["ppt/diagrams/unknown.xml"],
        parts["ppt/diagrams/unknown.xml"]
    );
}
#[test]
fn all_basic_public_effect_parameters_and_transitions_roundtrip() {
    let mut d = authoring_demo::deck();
    let target = d.slides[0].objects[1].id.clone();
    let effects = ["appear", "fade", "fade-out", "fly", "move", "zoom", "spin"]
        .iter()
        .enumerate()
        .map(|(i, kind)| Effect {
            id: format!("effect-{i}"),
            target: target.clone(),
            kind: (*kind).into(),
            trigger: ["click", "with", "after"][i % 3].into(),
            duration_ms: 700,
            delay_ms: 50,
            repeats: 2,
            auto_reverse: true,
            rewind: true,
            dx: 72.,
            dy: 36.,
            amount: 180.,
            ..Default::default()
        })
        .collect::<Vec<_>>();
    d.presentation.slides.get_mut("page-0").unwrap().effects = effects.clone();
    d.presentation.slides.get_mut("page-0").unwrap().transition = "push".into();
    d.presentation.slides.get_mut("page-0").unwrap().advance_ms = Some(5000);
    let imported = public(&d);
    let timing = imported
        .presentation
        .slides
        .values()
        .find(|t| t.transition == "push")
        .unwrap();
    assert_eq!(timing.advance_ms, Some(5000));
    assert_eq!(timing.effects.len(), 7);
    for (a, b) in effects.iter().zip(&timing.effects) {
        assert_eq!(
            (
                &a.kind,
                &a.trigger,
                a.duration_ms,
                a.delay_ms,
                a.repeats,
                a.auto_reverse,
                a.rewind
            ),
            (
                &b.kind,
                &b.trigger,
                b.duration_ms,
                b.delay_ms,
                b.repeats,
                b.auto_reverse,
                b.rewind
            )
        );
        if a.kind == "fly" || a.kind == "move" {
            assert!((a.dx - b.dx).abs() < 0.001 && (a.dy - b.dy).abs() < 0.001);
        }
        if a.kind == "spin" {
            assert_eq!(a.amount, b.amount);
        }
    }
}
#[test]
fn smartart_text_topology_import_is_explicit_simplification() {
    let frame=roxmltree::Document::parse(r#"<p:graphicFrame xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:d="http://schemas.openxmlformats.org/drawingml/2006/diagram" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><p:xfrm><a:off x="127000" y="254000"/><a:ext cx="8890000" cy="3302000"/></p:xfrm><d:relIds r:dm="data"/></p:graphicFrame>"#).unwrap();
    let mut parts = std::collections::BTreeMap::new();
    parts.insert("ppt/slides/_rels/slide1.xml.rels".into(),br#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="data" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/diagramData" Target="../diagrams/data1.xml"/></Relationships>"#.to_vec());
    parts.insert("ppt/diagrams/data1.xml".into(),br#"<d:dataModel xmlns:d="http://schemas.openxmlformats.org/drawingml/2006/diagram" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"><d:ptLst><d:pt modelId="a"><d:t><a:p><a:r><a:t>Plan</a:t></a:r></a:p></d:t></d:pt><d:pt modelId="b"><d:t><a:p><a:r><a:t>Build</a:t></a:r></a:p></d:t></d:pt></d:ptLst><d:cxnLst><d:cxn type="parOf" srcId="a" destId="b"/></d:cxnLst></d:dataModel>"#.to_vec());
    let o =
        joydeck::diagram_package::read(&parts, "ppt/slides/slide1.xml", frame.root_element(), 0)
            .unwrap();
    let d = o.diagram.unwrap();
    assert_eq!(d.layout, "hierarchy");
    assert_eq!(d.nodes[1].parent, Some("a".into()));
    assert_eq!((o.x, o.y), (10., 20.));
}
#[test]
fn morph_image_crop_interpolates_only_same_resource() {
    let mut d = authoring_demo::deck();
    let mut png = std::io::Cursor::new(Vec::new());
    image::DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(
        2,
        2,
        image::Rgba([230, 40, 20, 255]),
    ))
    .write_to(&mut png, image::ImageFormat::Png)
    .unwrap();
    let r = joydeck::media::Resource::from_png(png.get_ref()).unwrap();
    let id = r.id();
    d.resources.insert(id.clone(), Arc::new(r));
    for p in 0..2 {
        let mut o = edit::object("image");
        o.id = format!("image-{p}");
        o.morph_key = Some("picture".into());
        o.image = Some(joydeck::media::ImageRef {
            resource: id.clone(),
            crop: if p == 0 {
                [0., 0., 1., 1.]
            } else {
                [0.2, 0.1, 0.6, 0.7]
            },
        });
        d.slides[p].objects.push(o);
    }
    d.validate().unwrap();
    let f = motion::morph(&d, 0, 1, 0.5).unwrap();
    let crop = f.slides[1]
        .objects
        .iter()
        .find(|o| o.id == "image-1")
        .unwrap()
        .image
        .as_ref()
        .unwrap()
        .crop;
    assert!((crop[0] - 0.1).abs() < 1e-6 && (crop[2] - 0.8).abs() < 1e-6);
}

#[test]
fn empty_scatter_has_finite_axes_and_renders() {
    let mut d = authoring_demo::deck();
    let c = d.slides[4].objects[1].chart.as_mut().unwrap();
    c.series[0].values = vec![None; 4];
    c.series[0].x = vec![None; 4];
    d.validate().unwrap();
    assert!(preview::render(&d, 4, 960, &cancel()).is_ok());
}
