//! Independent static PresentationML writer. Native inheritance is resolved for public consumers.
use crate::{
    document::{Deck, Object, Result},
    interop::{escape, hash, validate_graph},
};
use base64::Engine;
use std::collections::BTreeMap;
const P: &str = "http://schemas.openxmlformats.org/presentationml/2006/main";
const A: &str = "http://schemas.openxmlformats.org/drawingml/2006/main";
const R: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";
const SNAP: &str = "customXml/joydeck.xml";
fn emu(n: f32) -> i64 {
    (n as f64 * 12700.).round() as i64
}
fn rels(items: &[(String, String, String)]) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">{}</Relationships>"#,
        items
            .iter()
            .map(|(id, kind, target)| format!(
                r#"<Relationship Id="{}" Type="{R}/{kind}" Target="{}"/>"#,
                escape(id),
                escape(target)
            ))
            .collect::<String>()
    )
}
fn tree() -> String {
    r#"<p:nvGrpSpPr><p:cNvPr id="1" name=""/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr><p:grpSpPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="0" cy="0"/><a:chOff x="0" y="0"/><a:chExt cx="0" cy="0"/></a:xfrm></p:grpSpPr>"#.into()
}
fn wrap(root: &str, body: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?><p:{root} xmlns:p="{P}" xmlns:a="{A}" xmlns:r="{R}">{body}</p:{root}>"#
    )
}
fn fill(c: &str, opacity: f32) -> String {
    format!(
        r#"<a:solidFill><a:srgbClr val="{}"><a:alpha val="{}"/></a:srgbClr></a:solidFill>"#,
        &c[1..],
        (opacity * 100000.).round() as u32
    )
}
fn xf(o: &Object) -> String {
    format!(
        r#"<a:xfrm rot="{}"><a:off x="{}" y="{}"/><a:ext cx="{}" cy="{}"/></a:xfrm>"#,
        (o.rotation * 60000.).round() as i64,
        emu(o.x),
        emu(o.y),
        emu(o.width),
        emu(o.height)
    )
}
fn runs(o: &Object) -> String {
    let mut paragraphs = String::new();
    let all = crate::text::runs(o);
    let mut start = 0;
    for line in o.text.split('\n') {
        let end = start + line.len();
        let mut runs = String::new();
        for r in &all {
            let a = r.start.max(start);
            let b = r.end.min(end);
            if a >= b {
                continue;
            }
            let s = &r.style;
            runs.push_str(&format!(r#"<a:r><a:rPr lang="zh-CN" sz="{}" b="{}" i="{}">{}<a:latin typeface="{}"/><a:ea typeface="{}"/></a:rPr><a:t xml:space="preserve">{}</a:t></a:r>"#,(s.size*100.).round() as u32,u8::from(s.bold),u8::from(s.italic),fill(&s.color,o.opacity),escape(&s.family),escape(&s.family),escape(&o.text[a..b])));
        }
        paragraphs.push_str(&format!(r#"<a:p><a:pPr><a:lnSpc><a:spcPct val="130000"/></a:lnSpc></a:pPr>{runs}<a:endParaRPr sz="{}"/></a:p>"#,(o.font_size*100.) as u32));
        start = end + 1;
    }
    format!(
        r#"<p:txBody><a:bodyPr wrap="none" lIns="0" rIns="0" tIns="0" bIns="0" anchor="t"/><a:lstStyle/>{paragraphs}</p:txBody>"#
    )
}
fn shape(o: &Object, id: usize) -> Result<String> {
    let name = escape(
        &o.morph_key
            .as_ref()
            .map_or_else(|| o.id.clone(), |key| format!("!!{key}")),
    );
    let transform = xf(o);
    let line = if o.stroke_width > 0. {
        format!(
            r#"<a:ln w="{}">{}</a:ln>"#,
            emu(o.stroke_width),
            fill(&o.stroke, o.opacity)
        )
    } else {
        "<a:ln><a:noFill/></a:ln>".into()
    };
    if o.kind == "image" {
        let i = o.image.as_ref().ok_or("图片缺少资源")?;
        let [x, y, w, h] = i.crop;
        return Ok(format!(
            r#"<p:pic><p:nvPicPr><p:cNvPr id="{id}" name="{name}"/><p:cNvPicPr/><p:nvPr/></p:nvPicPr><p:blipFill><a:blip r:embed="img{id}"><a:alphaModFix amt="{}"/></a:blip><a:srcRect l="{}" t="{}" r="{}" b="{}"/><a:stretch><a:fillRect/></a:stretch></p:blipFill><p:spPr>{transform}<a:prstGeom prst="rect"><a:avLst/></a:prstGeom>{line}</p:spPr></p:pic>"#,
            (o.opacity * 100000.) as u32,
            (x * 100000.).round() as i32,
            (y * 100000.).round() as i32,
            ((1. - x - w) * 100000.).round().max(0.) as i32,
            ((1. - y - h) * 100000.).round().max(0.) as i32
        ));
    }
    let geom = match o.kind.as_str() {
        "rect" | "text" => "rect",
        "ellipse" => "ellipse",
        "roundRect" => "roundRect",
        "arrow" => "rightArrow",
        "line" => "line",
        _ => return Err(format!("PPTX导出尚不支持对象 {}", o.kind)),
    };
    let paint = if o.kind == "text" || o.kind == "line" {
        "<a:noFill/>".into()
    } else {
        fill(&o.fill, o.opacity)
    };
    let line = if o.kind == "line" {
        format!(
            r#"<a:ln w="{}">{}</a:ln>"#,
            emu(o.stroke_width.max(1.)),
            fill(&o.fill, o.opacity)
        )
    } else {
        line
    };
    let text = if o.kind == "text" {
        runs(o)
    } else {
        String::new()
    };
    Ok(format!(
        r#"<p:sp><p:nvSpPr><p:cNvPr id="{id}" name="{name}"/><p:cNvSpPr txBox="{}"/><p:nvPr/></p:nvSpPr><p:spPr>{transform}<a:prstGeom prst="{geom}"><a:avLst/></a:prstGeom>{paint}{line}</p:spPr>{text}</p:sp>"#,
        u8::from(o.kind == "text")
    ))
}
fn diagram_group(o: &Object, id: usize) -> Result<String> {
    let mut local = o.clone();
    local.rotation = 0.;
    let mut children = String::new();
    for (i, child) in crate::diagram::expand(&local)?.iter().enumerate() {
        children += &shape(child, 1_000_000 + id * 256 + i)?;
    }
    Ok(format!(
        r#"<p:grpSp><p:nvGrpSpPr><p:cNvPr id="{id}" name="{}"/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr><p:grpSpPr><a:xfrm rot="{}"><a:off x="{}" y="{}"/><a:ext cx="{}" cy="{}"/><a:chOff x="{}" y="{}"/><a:chExt cx="{}" cy="{}"/></a:xfrm></p:grpSpPr>{children}</p:grpSp>"#,
        escape(&o.id),
        (o.rotation * 60000.) as i64,
        emu(o.x),
        emu(o.y),
        emu(o.width),
        emu(o.height),
        emu(o.x),
        emu(o.y),
        emu(o.width),
        emu(o.height)
    ))
}
pub fn losses(deck: &Deck) -> Vec<String> {
    let mut l=vec!["公开PPTX重新生成已解析对象与基础时间线；母版/主题继承、节和动态字段会固化，专有快照仅供未被外部改写的文件在JoyDeck完整重开".into(),"外部应用字体与排版可能有差异；本机原创基础样例已验证，不代表任意PPTX".into()];
    if !deck.playback.clips.is_empty() {
        l.push("媒体仅在JoyDeck原生文件保留；公开PPTX不含可播放音视频".into());
    }
    if !deck.review.threads.is_empty() || !deck.review.accessibility.is_empty() {
        l.push("评论与阅读顺序仅在JoyDeck原生文件保留；公开PPTX暂不生成评论或alt text".into());
    }
    if deck.source.is_some() {
        l.push("新建导出不携带未解析的扩展部件、复杂动画/chartex/SmartArt/OLE；基础图表和时间线会重新生成。需要保留原包时使用源包写回".into());
    }
    if deck
        .slides
        .iter()
        .flat_map(|s| &s.objects)
        .any(|o| o.diagram.is_some())
    {
        l.push("基础图示公开输出为可编辑形状组合；自动布局与文本层级仅在JoyDeck快照中保留".into());
    }
    if !deck.presentation.links.is_empty() {
        l.push("对象放映链接在JoyDeck原生文稿中生效；公开PPTX暂未写入这些跳转链接".into());
    }
    if !deck.presentation.slides.is_empty() {
        l.push("公开输出支持基础动画与转场；Morph对象名称匹配、自动布局、复杂文本效果和自定义顺序在其他软件中可能不同".into());
    }
    l
}
pub fn generate(deck: &Deck) -> Result<BTreeMap<String, Vec<u8>>> {
    deck.validate()?;
    let mut parts = BTreeMap::new();
    let mut overrides: Vec<(String, String)> = vec![
        (
            "/ppt/presentation.xml".into(),
            "presentation.main+xml".into(),
        ),
        (
            "/ppt/slideMasters/slideMaster1.xml".into(),
            "slideMaster+xml".into(),
        ),
        (
            "/ppt/slideLayouts/slideLayout1.xml".into(),
            "slideLayout+xml".into(),
        ),
    ];
    let mut mainrels = vec![
        (
            "master".into(),
            "slideMaster".into(),
            "slideMasters/slideMaster1.xml".into(),
        ),
        (
            "snapshot".into(),
            "customXml".into(),
            "../customXml/joydeck.xml".into(),
        ),
    ];
    let mut ids = String::new();
    for (i, original) in deck.slides.iter().enumerate() {
        let n = i + 1;
        let s = crate::design::resolve_slide(deck, i)?;
        let mut objects = tree();
        let mut slide_rels = vec![(
            "layout".into(),
            "slideLayout".into(),
            "../slideLayouts/slideLayout1.xml".into(),
        )];
        for (j, o) in s.objects.iter().enumerate() {
            objects.push_str(&if o.kind == "diagram" {
                diagram_group(o, j + 2)?
            } else if o.kind == "chart" {
                let name = format!("chart-{n}-{}", j + 2);
                let mut c = o.chart.clone().ok_or("图表模型缺失")?;
                for color in &mut c.colors {
                    *color = crate::table::theme_value(deck, i, color, false);
                }
                let (xml, xlsx, rels) = crate::chart_package::write(&c, &name)?;
                parts.insert(format!("ppt/charts/{name}.xml"), xml.into_bytes());
                parts.insert(
                    format!("ppt/charts/_rels/{name}.xml.rels"),
                    rels.into_bytes(),
                );
                parts.insert(format!("ppt/embeddings/{name}.xlsx"), xlsx);
                overrides.push((format!("/ppt/charts/{name}.xml"), "chart+xml".into()));
                slide_rels.push((
                    name.clone(),
                    "chart".into(),
                    format!("../charts/{name}.xml"),
                ));
                crate::chart_package::frame(o, j + 2, &name)
            } else if o.kind == "table" {
                crate::table_pptx::write(deck, i, o, j + 2)?
            } else {
                shape(o, j + 2)?
            });
            if let Some(image) = &o.image {
                slide_rels.push((
                    format!("img{}", j + 2),
                    "image".into(),
                    format!("../media/{}.png", image.resource),
                ));
            }
        }
        let content = wrap(
            "sld",
            &format!(
                r#"<p:cSld name="{}"><p:bg><p:bgPr>{}<a:effectLst/></p:bgPr></p:bg><p:spTree>{objects}</p:spTree></p:cSld><p:clrMapOvr><a:masterClrMapping/></p:clrMapOvr>"#,
                escape(&s.title),
                fill(&s.background, 1.)
            ),
        );
        let motion = crate::motion_package::write(
            &s,
            &deck.presentation.timing(&original.id),
            deck.width,
            deck.height,
        )?;
        let content = content.replace("</p:sld>", &format!("{motion}</p:sld>"));
        let content = content.replacen(
            "<p:sld ",
            &format!("<p:sld show=\"{}\" ", u8::from(!s.hidden)),
            1,
        );
        parts.insert(format!("ppt/slides/slide{n}.xml"), content.into_bytes());
        overrides.push((format!("/ppt/slides/slide{n}.xml"), "slide+xml".into()));
        mainrels.push((
            format!("slide{n}"),
            "slide".into(),
            format!("slides/slide{n}.xml"),
        ));
        ids.push_str(&format!(r#"<p:sldId id="{}" r:id="slide{n}"/>"#, 256 + i));
        if !original.notes.is_empty() {
            let mut o = crate::edit::object("text");
            o.id = "notes".into();
            o.text = original.notes.clone();
            let xml = wrap(
                "notes",
                &format!(
                    r#"<p:cSld><p:spTree>{}{}</p:spTree></p:cSld><p:clrMapOvr><a:masterClrMapping/></p:clrMapOvr>"#,
                    tree(),
                    shape(&o, 2)?.replace(
                        "<p:nvPr/>",
                        "<p:nvPr><p:ph type=\"body\" idx=\"1\"/></p:nvPr>"
                    )
                ),
            );
            parts.insert(
                format!("ppt/notesSlides/notesSlide{n}.xml"),
                xml.into_bytes(),
            );
            parts.insert(
                format!("ppt/notesSlides/_rels/notesSlide{n}.xml.rels"),
                rels(&[(
                    "slide".into(),
                    "slide".into(),
                    format!("../slides/slide{n}.xml"),
                )])
                .into_bytes(),
            );
            slide_rels.push((
                "notes".into(),
                "notesSlide".into(),
                format!("../notesSlides/notesSlide{n}.xml"),
            ));
            overrides.push((
                format!("/ppt/notesSlides/notesSlide{n}.xml"),
                "notesSlide+xml".into(),
            ));
        }
        parts.insert(
            format!("ppt/slides/_rels/slide{n}.xml.rels"),
            rels(&slide_rels).into_bytes(),
        );
    }
    let cm = r#"<p:clrMap accent1="accent1" accent2="accent2" accent3="accent3" accent4="accent4" accent5="accent5" accent6="accent6" bg1="lt1" bg2="lt2" folHlink="folHlink" hlink="hlink" tx1="dk1" tx2="dk2"/>"#;
    parts.insert("ppt/presentation.xml".into(),wrap("presentation",&format!(r#"<p:sldMasterIdLst><p:sldMasterId id="2147483648" r:id="master"/></p:sldMasterIdLst><p:sldIdLst>{ids}</p:sldIdLst><p:sldSz cx="{}" cy="{}"/><p:notesSz cx="6858000" cy="9144000"/>"#,emu(deck.width),emu(deck.height))).into_bytes());
    parts.insert(
        "ppt/_rels/presentation.xml.rels".into(),
        rels(&mainrels).into_bytes(),
    );
    parts.insert(
        "_rels/.rels".into(),
        rels(&[(
            "office".into(),
            "officeDocument".into(),
            "ppt/presentation.xml".into(),
        )])
        .into_bytes(),
    );
    parts.insert("ppt/slideMasters/slideMaster1.xml".into(),wrap("sldMaster",&format!(r#"<p:cSld><p:spTree>{}</p:spTree></p:cSld>{cm}<p:sldLayoutIdLst><p:sldLayoutId id="2147483649" r:id="layout"/></p:sldLayoutIdLst><p:txStyles><p:titleStyle/><p:bodyStyle/><p:otherStyle/></p:txStyles>"#,tree())).into_bytes());
    parts.insert(
        "ppt/slideMasters/_rels/slideMaster1.xml.rels".into(),
        rels(&[
            (
                "layout".into(),
                "slideLayout".into(),
                "../slideLayouts/slideLayout1.xml".into(),
            ),
            ("theme".into(), "theme".into(), "../theme/theme1.xml".into()),
        ])
        .into_bytes(),
    );
    parts.insert("ppt/slideLayouts/slideLayout1.xml".into(),wrap("sldLayout",&format!(r#"<p:cSld name="JoyDeck Blank"><p:spTree>{}</p:spTree></p:cSld><p:clrMapOvr><a:masterClrMapping/></p:clrMapOvr>"#,tree())).into_bytes());
    parts.insert(
        "ppt/slideLayouts/_rels/slideLayout1.xml.rels".into(),
        rels(&[(
            "master".into(),
            "slideMaster".into(),
            "../slideMasters/slideMaster1.xml".into(),
        )])
        .into_bytes(),
    );
    let mut colors = String::new();
    for (name, c) in [
        ("dk1", "202b3a"),
        ("lt1", "ffffff"),
        ("dk2", "15202d"),
        ("lt2", "eeeeee"),
        ("accent1", "087f8c"),
        ("accent2", "1265c5"),
        ("accent3", "c05050"),
        ("accent4", "808040"),
        ("accent5", "804080"),
        ("accent6", "408080"),
        ("hlink", "0000ff"),
        ("folHlink", "800080"),
    ] {
        colors.push_str(&format!(r#"<a:{name}><a:srgbClr val="{c}"/></a:{name}>"#));
    }
    let fills = (0..3)
        .map(|_| r#"<a:solidFill><a:schemeClr val="phClr"/></a:solidFill>"#)
        .collect::<String>();
    let lines=(0..3).map(|_|r#"<a:ln w="12700"><a:solidFill><a:schemeClr val="phClr"/></a:solidFill><a:prstDash val="solid"/></a:ln>"#).collect::<String>();
    parts.insert("ppt/theme/theme1.xml".into(),format!(r#"<?xml version="1.0" encoding="UTF-8"?><a:theme xmlns:a="{A}" name="JoyDeck Original"><a:themeElements><a:clrScheme name="JoyDeck">{colors}</a:clrScheme><a:fontScheme name="JoyDeck"><a:majorFont><a:latin typeface="Arial"/><a:ea typeface=""/><a:cs typeface=""/></a:majorFont><a:minorFont><a:latin typeface="Arial"/><a:ea typeface=""/><a:cs typeface=""/></a:minorFont></a:fontScheme><a:fmtScheme name="JoyDeck"><a:fillStyleLst>{fills}</a:fillStyleLst><a:lnStyleLst>{lines}</a:lnStyleLst><a:effectStyleLst><a:effectStyle><a:effectLst/></a:effectStyle><a:effectStyle><a:effectLst/></a:effectStyle><a:effectStyle><a:effectLst/></a:effectStyle></a:effectStyleLst><a:bgFillStyleLst>{fills}</a:bgFillStyleLst></a:fmtScheme></a:themeElements></a:theme>"#).into_bytes());
    for (id, r) in &deck.resources {
        parts.insert(format!("ppt/media/{id}.png"), r.png.clone());
    }
    let mut ct = r#"<?xml version="1.0" encoding="UTF-8"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Default Extension="png" ContentType="image/png"/><Default Extension="xlsx" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sheet"/><Override PartName="/ppt/theme/theme1.xml" ContentType="application/vnd.openxmlformats-officedocument.theme+xml"/>"#.to_owned();
    for (path, kind) in overrides {
        let mime = if kind == "chart+xml" {
            "application/vnd.openxmlformats-officedocument.drawingml.chart+xml".to_owned()
        } else {
            format!("application/vnd.openxmlformats-officedocument.presentationml.{kind}")
        };
        ct.push_str(&format!(
            r#"<Override PartName="{path}" ContentType="{mime}"/>"#
        ));
    }
    ct.push_str("</Types>");
    parts.insert("[Content_Types].xml".into(), ct.into_bytes());
    let digest = hash(&parts);
    let mut snapshot = deck.clone();
    snapshot.source = None;
    let data = serde_json::to_vec(&snapshot).map_err(|e| e.to_string())?;
    let encoded = base64::engine::general_purpose::STANDARD.encode(data);
    parts.insert(SNAP.into(),format!(r#"<jd:document xmlns:jd="urn:joydeck:interop:1" version="1" digest="{digest}">{encoded}</jd:document>"#).into_bytes());
    validate_graph(&parts)?;
    Ok(parts)
}
pub fn read_snapshot(parts: &BTreeMap<String, Vec<u8>>) -> Result<Option<Deck>> {
    let Some(bytes) = parts.get(SNAP) else {
        return Ok(None);
    };
    let doc = crate::interop::xml(std::str::from_utf8(bytes).map_err(|e| e.to_string())?)?;
    let root = doc.root_element();
    if !root.has_tag_name(("urn:joydeck:interop:1", "document"))
        || root.attribute("version") != Some("1")
    {
        return Ok(None);
    }
    let mut public = parts.clone();
    public.remove(SNAP);
    if root.attribute("digest") != Some(hash(&public).as_str()) {
        return Ok(None);
    }
    let data = base64::engine::general_purpose::STANDARD
        .decode(root.text().unwrap_or(""))
        .map_err(|e| e.to_string())?;
    if data.len() > crate::document::MAX_PART {
        return Err("原生快照超限".into());
    }
    let deck: Deck = serde_json::from_slice(&data).map_err(|e| e.to_string())?;
    if deck.source.is_some() {
        return Err("交换快照不允许递归源包".into());
    }
    deck.validate()?;
    Ok(Some(deck))
}
