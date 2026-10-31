//! Original table sample, available independently of validation harnesses.
use super::*;
use joydeck::table::{self, Action};
pub(super) fn seed(s: &mut Editor, w: &mut Window, cx: &mut Context<Editor>) {
    s.session = Session::new();
    s.renew_recovery();
    s.page = 0;
    let mut title = edit::object("text");
    title.x = 64.;
    title.y = 42.;
    title.width = 820.;
    title.height = 48.;
    title.text = "季度业务回顾".into();
    title.fill = "#25334b".into();
    title.font_size = 32.;
    let mut sub = title.clone();
    sub.y = 94.;
    sub.height = 32.;
    sub.font_size = 14.;
    sub.fill = "#8490a5".into();
    sub.text = "FY 2026  /  运营表现与下一阶段重点".into();
    s.command(
        Command::Batch(vec![
            Command::AddObject(0, title),
            Command::AddObject(0, sub),
        ]),
        cx,
    );
    let mut o = match table::object(4, 5) {
        Ok(object) => object,
        Err(error) => {
            s.message = error;
            cx.notify();
            return;
        }
    };
    o.x = 64.;
    o.y = 166.;
    o.width = 832.;
    o.height = 240.;
    table::resize(&mut o);
    s.command(Command::AddObject(0, o), cx);
    let id = s.session.deck.slides[0].objects[2].id.clone();
    s.command(Command::Table(0,id.clone(),Action::Paste(0,0,"业务板块\t目标 / 万\t实际 / 万\t完成率\t同比增长\n企业服务\t120\t136\t113%\t+24%\n软件订阅\t240\t268\t112%\t+32%\n专业支持\t80\t92\t115%\t+18%".into())),cx);
    let mut note = edit::object("text");
    note.x = 64.;
    note.y = 441.;
    note.width = 832.;
    note.height = 40.;
    note.text = "核心业务持续增长，订阅收入占比进一步提升。".into();
    note.font_size = 16.;
    note.fill = "#69788f".into();
    s.command(Command::AddObject(0, note), cx);
    s.command(
        Command::Design(joydeck::design_commands::Action::Properties(
            "季度业务回顾".into(),
            joydeck::design::Organization {
                author: "JoyDeck".into(),
                subject: "原创表格验收样例".into(),
                ..Default::default()
            },
            "2026-10-30".into(),
        )),
        cx,
    );
    s.select(2, w, cx);
    s.table_pick = Some(table_ui::Pick {
        id,
        anchor: (0, 0),
        focus: (0, 0),
    });
    s.ribbon = "表格".into();
    s.changed(cx);
}
