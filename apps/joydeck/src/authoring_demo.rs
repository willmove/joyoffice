//! Original, redistributable authoring and presentation acceptance document.
use crate::{
    chart, diagram,
    document::{Deck, Slide},
    edit,
    motion::{Effect, Timing},
};
pub fn deck() -> Deck {
    let mut d = (*edit::Session::new().deck).clone();
    d.title = "季度业务回顾".into();
    d.slides.clear();
    for (i, kind) in [
        "column",
        "bar",
        "line",
        "pie",
        "scatter",
        "process",
        "hierarchy",
        "area",
        "doughnut",
    ]
    .iter()
    .enumerate()
    {
        let mut title = edit::object("text");
        title.id = format!("title-{i}");
        title.x = 54.;
        title.y = 32.;
        title.width = 850.;
        title.height = 50.;
        title.text = if !(5..7).contains(&i) {
            format!(
                "季度业务表现 · {}",
                [
                    "柱形图",
                    "条形图",
                    "折线图",
                    "饼图",
                    "散点图",
                    "流程",
                    "层级",
                    "面积图",
                    "环形图"
                ][i]
            )
        } else {
            "从计划到成果".into()
        };
        title.font_size = 30.;
        title.fill = "#20334d".into();
        let mut o = if !(5..7).contains(&i) {
            chart::object(kind)
        } else {
            diagram::object(kind)
        };
        o.id = format!("content-{i}");
        o.x = 74.;
        o.y = 122.;
        o.width = 812.;
        o.height = 330.;
        if let Some(c) = &mut o.chart {
            c.title = String::new();
            c.labels = true;
            c.unit = " 万元".into();
            if *kind == "scatter" {
                c.series[0].x = vec![Some(1.), Some(3.), Some(7.), Some(12.)];
                c.x_title = "投入".into();
            }
        }
        if *kind == "hierarchy" {
            o.diagram
                .as_mut()
                .unwrap()
                .set_text("业务计划\n\t客户增长\n\t\t客户体验\n\t产品迭代")
                .unwrap();
        }
        let mut footer = edit::object("text");
        footer.id = format!("footer-{i}");
        footer.x = 54.;
        footer.y = 490.;
        footer.width = 850.;
        footer.height = 25.;
        footer.font_size = 13.;
        footer.fill = "#777777".into();
        footer.text = "FY 2026    /    JoyDeck 原创开发验证样例".into();
        let id = format!("page-{i}");
        d.slides.push(Slide {
            id: id.clone(),
            title: title.text.clone(),
            background: "#ffffff".into(),
            hidden: false,
            layout: None,
            inherit_background: false,
            section: None,
            notes: format!("私密演讲者备注：第{}页，仅在演示者窗口可见。", i + 1),
            objects: vec![title, o, footer],
        });
        d.presentation.slides.insert(
            id,
            Timing {
                transition: if i == 0 { "none" } else { "fade" }.into(),
                effects: vec![Effect {
                    id: format!("entrance-{i}"),
                    target: format!("content-{i}"),
                    duration_ms: 600,
                    ..Default::default()
                }],
                ..Default::default()
            },
        );
    }
    d.validate().unwrap();
    d
}
