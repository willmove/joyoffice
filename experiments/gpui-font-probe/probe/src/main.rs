//! W02 探针入口：两个模式。
//!
//! 1. 窗口模式（默认）：加载系统 CJK 字体 → 改写 name 表为唯一家族名
//!    → 用改写后的字节整形并构建 A4 场景 → add_fonts 注册同一份字节
//!    → resolve_font 唯一家族名 → `Window::paint_glyph` 按场景字形
//!    id/位置绘制（含标定块、基线与每字形红色标尺，供截图后做机器
//!    几何比对）；同一场景同一字节另出 PDF。
//! 2. `--conflict-check`：无窗口。度量按名解析在"注册顶替字体（不同
//!    字节、同名）"前后是否漂移，验证系统同名字体冲突；同时以唯一
//!    家族名做正控制。
//!
//! 界面顶部的说明文字由 GPUI 系统字体渲染，只是界面说明；链路验证的
//! 正文全部由 paint_glyph 按外部整形结果绘制。

mod capture;
mod conflict;
mod crosscheck;

use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use gpui::{
    App, Application, Bounds, Context, FocusHandle, Font, FontFeatures, FontId, FontStyle,
    FontWeight, GlyphId, Hsla, KeyBinding, Pixels, Point, Window, WindowBounds, WindowOptions,
    actions, canvas, div, fill, outline, point, prelude::*, px, rgb, size,
};
use joyword_foundation::Emu;
use joyword_layout::GlyphRun;

use gpui_font_probe::{
    DEFAULT_FACE, DEFAULT_FONT, LINES, ProbeFont, build_scene, emu_to_px, glyph_origin_px, json,
};

actions!(probe, [Zoom100, Zoom150, Zoom200, ResetView]);

/// 画布与标定几何（逻辑像素，窗口坐标系）。
const CANVAS_W: f64 = 1000.0;
const CANVAS_H: f64 = 760.0;
const CAL_SIZE: f64 = 8.0;
/// 四个标定块：前三个黑色，第四个红色（确定方向与物理缩放）。
const CAL_POINTS: [(f64, f64); 4] = [(24.0, 24.0), (956.0, 24.0), (24.0, 688.0), (956.0, 688.0)];
const PAGE_X: f64 = 48.0;
const PAGE_Y: f64 = 64.0;
/// 每行的红色字形原点标尺放在基线上方这个距离（避开字身）。
const RULER_LIFT: f64 = 30.0;

struct Probe {
    font: Arc<ProbeFont>,
    plan_source: PlanSource,
    font_id: FontId,
    zoom: f64,
    evidence_out: PathBuf,
    evidence_written: Arc<AtomicBool>,
    focus_handle: FocusHandle,
}

/// 生成绘制计划所需的场景数据（自 hold 的 GlyphRun 与页面尺寸）。
struct PlanSource {
    runs: Vec<GlyphRun>,
    page_w_emu: i64,
    page_h_emu: i64,
}

#[derive(Clone)]
struct PlacedGlyph {
    index: usize,
    glyph_id: u16,
    origin_x: f64,
    origin_y: f64,
}

#[derive(Clone)]
struct PaintRun {
    line: usize,
    origin_x: f64,
    baseline_y: f64,
    ruler_y: f64,
    font_size: f32,
    glyphs: Vec<PlacedGlyph>,
}

#[derive(Clone)]
struct PaintPlan {
    zoom: f64,
    scale_factor: f64,
    page_w_emu: i64,
    page_h_emu: i64,
    page_w: f64,
    page_h: f64,
    runs: Vec<PaintRun>,
}

fn black() -> Hsla {
    rgb(0x000000).into()
}

fn red() -> Hsla {
    rgb(0xff0000).into()
}

fn gray(alpha: f32) -> Hsla {
    gpui::hsla(0.0, 0.0, 0.5, alpha)
}

struct Args {
    font: PathBuf,
    face: u32,
    zoom: f64,
    evidence_out: PathBuf,
    quit_after: Option<Duration>,
    conflict_check: bool,
    glyph_crosscheck: bool,
}

fn parse_args() -> Args {
    let mut args = Args {
        font: PathBuf::from(DEFAULT_FONT),
        face: DEFAULT_FACE,
        zoom: 1.0,
        evidence_out: PathBuf::from("artifacts"),
        quit_after: None,
        conflict_check: false,
        glyph_crosscheck: false,
    };
    let mut iter = std::env::args().skip(1);
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--font" => args.font = PathBuf::from(iter.next().expect("--font needs a path")),
            "--face" => {
                args.face = iter
                    .next()
                    .expect("--face needs a number")
                    .parse()
                    .expect("face index")
            }
            "--zoom" => {
                args.zoom = iter
                    .next()
                    .expect("--zoom needs a number")
                    .parse()
                    .expect("zoom")
            }
            "--evidence-out" => {
                args.evidence_out = PathBuf::from(iter.next().expect("--evidence-out needs a path"))
            }
            "--quit-after-secs" => {
                args.quit_after = Some(Duration::from_secs_f64(
                    iter.next()
                        .expect("--quit-after-secs needs a number")
                        .parse()
                        .expect("seconds"),
                ))
            }
            "--conflict-check" => args.conflict_check = true,
            "--glyph-crosscheck" => args.glyph_crosscheck = true,
            other => panic!("unknown argument {other}"),
        }
    }
    args
}

fn probe_font(family: &str) -> Font {
    Font {
        family: gpui::SharedString::from(family.to_owned()),
        features: FontFeatures::default(),
        fallbacks: None,
        weight: FontWeight::default(),
        style: FontStyle::default(),
    }
}

fn main() {
    let args = parse_args();
    if args.conflict_check {
        conflict::run(&args.evidence_out);
        return;
    }
    if args.glyph_crosscheck {
        crosscheck::run(&args.evidence_out);
        return;
    }
    let font = Arc::new(
        gpui_font_probe::load_probe_font(&args.font, args.face)
            .unwrap_or_else(|error| panic!("probe font: {error}")),
    );
    std::fs::create_dir_all(&args.evidence_out).expect("create evidence dir");

    // 场景与 PDF 都只用改写后的字节；整形在 build_scene 内完成。
    let scene = build_scene(&font.loaded);
    let pdf = joyword_pdf::render(
        std::slice::from_ref(&scene),
        std::slice::from_ref(&font.loaded),
    )
    .expect("render pdf");
    std::fs::write(args.evidence_out.join("probe.pdf"), &pdf).expect("write pdf");

    let plan_source = PlanSource {
        runs: scene.runs.clone(),
        page_w_emu: scene.width.raw(),
        page_h_emu: scene.height.raw(),
    };
    let zoom = args.zoom;
    let evidence_out = args.evidence_out.clone();
    let quit_after = args.quit_after;

    Application::new().run(move |cx: &mut App| {
        let ts = cx.text_system();
        ts.add_fonts(vec![std::borrow::Cow::Owned(font.rewritten.to_vec())])
            .expect("add_fonts of the rewritten bytes");
        let font_id = ts.resolve_font(&probe_font(&font.names.family));

        cx.bind_keys(vec![
            KeyBinding::new("1", Zoom100, None),
            KeyBinding::new("2", Zoom150, None),
            KeyBinding::new("3", Zoom200, None),
            KeyBinding::new("0", ResetView, None),
        ]);

        let bounds = Bounds::centered(None, size(px(CANVAS_W as f32), px(CANVAS_H as f32)), cx);
        let opened = cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: Some(gpui::TitlebarOptions {
                    title: Some("W02 GPUI 字体链路探针".into()),
                    ..Default::default()
                }),
                ..Default::default()
            },
            |_, cx| {
                cx.new(|cx| {
                    cx.focus_handle();
                    Probe {
                        font: font.clone(),
                        plan_source,
                        font_id,
                        zoom,
                        evidence_out: evidence_out.clone(),
                        evidence_written: Arc::new(AtomicBool::new(false)),
                        focus_handle: cx.focus_handle(),
                    }
                })
            },
        );
        match opened {
            Ok(window) => {
                let _ = window.update(cx, |view, window, cx| {
                    window.focus(&view.focus_handle);
                    cx.activate(true);
                });
                if let Some(delay) = quit_after {
                    cx.spawn(async move |cx| {
                        cx.background_executor().timer(delay).await;
                        let _ = cx.update(|cx| cx.quit());
                    })
                    .detach();
                }
            }
            Err(error) => {
                eprintln!("could not open the probe window: {error}");
                cx.quit();
            }
        }
    });
}

/// 键盘缩放只影响观察视图；证据 JSON 按当次绘制实际使用的 zoom 写出。
mod zoom_choice {
    use std::cell::Cell;

    thread_local! {
        static CHOICE: Cell<Option<f64>> = const { Cell::new(None) };
    }

    pub fn set(zoom: f64) {
        CHOICE.with(|cell| cell.set(Some(zoom)));
    }

    pub fn get(default: f64) -> f64 {
        CHOICE.with(|cell| cell.get().unwrap_or(default))
    }
}

impl Probe {
    fn paint_plan(&self, zoom: f64) -> PaintPlan {
        let runs = self
            .plan_source
            .runs
            .iter()
            .enumerate()
            .map(|(line, run)| PaintRun {
                line,
                origin_x: PAGE_X + emu_to_px(run.origin_x, zoom),
                baseline_y: PAGE_Y + emu_to_px(run.baseline_y, zoom),
                ruler_y: PAGE_Y + emu_to_px(run.baseline_y, zoom) - RULER_LIFT * zoom,
                font_size: emu_to_px(run.size, zoom) as f32,
                glyphs: (0..run.glyphs.len())
                    .map(|index| {
                        let (x, y) = glyph_origin_px(run, index, zoom);
                        PlacedGlyph {
                            index,
                            glyph_id: run.glyphs[index].glyph_id,
                            origin_x: PAGE_X + x,
                            origin_y: PAGE_Y + y,
                        }
                    })
                    .collect(),
            })
            .collect();
        PaintPlan {
            zoom,
            scale_factor: 0.0,
            page_w_emu: self.plan_source.page_w_emu,
            page_h_emu: self.plan_source.page_h_emu,
            page_w: emu_to_px(Emu::new(self.plan_source.page_w_emu), zoom),
            page_h: emu_to_px(Emu::new(self.plan_source.page_h_emu), zoom),
            runs,
        }
    }
}

impl Render for Probe {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let zoom = zoom_choice::get(self.zoom);
        self.zoom = zoom;
        let plan = self.paint_plan(zoom);
        let font_id = self.font_id;
        let font = self.font.clone();
        let evidence_out = self.evidence_out.clone();
        let evidence_written = self.evidence_written.clone();
        let focus = self.focus_handle.clone();
        let header1 = format!(
            "W02 探针 · 家族 {} · 原始 sha256/12 {} · face {} · FontId {}",
            self.font.names.family,
            &self.font.original_sha256[..12],
            self.font.face_index,
            font_id.0,
        );
        let header2 = format!(
            "正文由 paint_glyph 按外部整形结果绘制 · zoom {zoom} · A4 场景投影 · 键 1/2/3/0 缩放",
        );
        div()
            .track_focus(&focus)
            .size_full()
            .flex()
            .flex_col()
            .bg(rgb(0xf2f4f7))
            .on_action(cx.listener(|_, _: &Zoom100, _, cx| {
                zoom_choice::set(1.0);
                cx.notify();
            }))
            .on_action(cx.listener(|_, _: &Zoom150, _, cx| {
                zoom_choice::set(1.5);
                cx.notify();
            }))
            .on_action(cx.listener(|_, _: &Zoom200, _, cx| {
                zoom_choice::set(2.0);
                cx.notify();
            }))
            .on_action(cx.listener(|_, _: &ResetView, _, cx| {
                zoom_choice::set(1.0);
                cx.notify();
            }))
            .child(
                div()
                    .px_4()
                    .py_2()
                    .bg(rgb(0x163b65))
                    .text_color(rgb(0xffffff))
                    .text_sm()
                    .child(header1)
                    .child(header2),
            )
            .child(
                div().flex_1().min_h_0().overflow_hidden().child(
                    canvas(
                        move |_, _, _| {},
                        move |bounds, _, window, cx| {
                            let mut plan = plan.clone();
                            plan.scale_factor = f64::from(window.scale_factor());
                            paint_scene(window, bounds, &plan, font_id);
                            if !evidence_written.swap(true, Ordering::SeqCst) {
                                write_evidence(window, &plan, &font, &evidence_out);
                                // 当前帧尚未合成；延后约 600ms 再按窗口 ID
                                // 捕获本进程窗口（见 capture.rs）。
                                let out_dir = evidence_out.clone();
                                let zoom = plan.zoom;
                                cx.spawn(async move |cx| {
                                    cx.background_executor()
                                        .timer(Duration::from_millis(600))
                                        .await;
                                    let name = format!("window-z{:02}.png", zoom * 100.0);
                                    let path = out_dir.join(&name);
                                    let note = match capture::capture_own_window_png(
                                        CANVAS_W, CANVAS_H, &path,
                                    ) {
                                        Ok(()) => format!("ok {}", name),
                                        Err(error) => format!("failed: {error}"),
                                    };
                                    eprintln!("window capture {note}");
                                    let _ = std::fs::write(
                                        out_dir.join("window-capture.txt"),
                                        format!("zoom {zoom}: {note}\n"),
                                    );
                                })
                                .detach();
                            }
                        },
                    )
                    .size_full(),
                ),
            )
    }
}

fn rect(x: f64, y: f64, w: f64, h: f64) -> gpui::Bounds<Pixels> {
    gpui::Bounds {
        origin: point(px(x as f32), px(y as f32)),
        size: size(px(w as f32), px(h as f32)),
    }
}

fn paint_scene(
    window: &mut Window,
    bounds: gpui::Bounds<Pixels>,
    plan: &PaintPlan,
    font_id: FontId,
) {
    // 所有几何以 canvas 元素原点为参考（画布逻辑坐标），窗口绝对
    // 坐标由 bounds.origin 平移得到；证据 JSON 记录的是画布坐标。
    let ox = f64::from(bounds.origin.x);
    let oy = f64::from(bounds.origin.y);
    let at = |x: f64, y: f64| (ox + x, oy + y);

    // 画布底与标定块（前三个黑色、第四个红色）。
    window.paint_quad(fill(
        rect(
            ox,
            oy,
            f64::from(bounds.size.width),
            f64::from(bounds.size.height),
        ),
        rgb(0xffffff),
    ));
    for (index, (x, y)) in CAL_POINTS.iter().enumerate() {
        let color = if index == 3 { red() } else { black() };
        let (sx, sy) = at(*x, *y);
        window.paint_quad(fill(rect(sx, sy, CAL_SIZE, CAL_SIZE), color));
    }
    // 页面（白底灰框）。
    let (px0, py0) = at(PAGE_X, PAGE_Y);
    window.paint_quad(fill(
        rect(px0, py0, plan.page_w, plan.page_h),
        rgb(0xfdfdff),
    ));
    window.paint_quad(outline(
        rect(px0, py0, plan.page_w, plan.page_h),
        gray(0.8),
        gpui::BorderStyle::Solid,
    ));
    for run in &plan.runs {
        // 基线与行起点标记。
        let (bx, by) = at(run.origin_x, run.baseline_y);
        let baseline_w = plan.page_w - (run.origin_x - PAGE_X);
        window.paint_quad(fill(rect(bx, by, baseline_w, 1.0), gray(0.45)));
        window.paint_quad(fill(rect(bx, by - 4.0, 2.0, 4.0), gray(0.9)));
        // 每字形红色原点标尺（x 即字形原点 x）。
        for glyph in &run.glyphs {
            let (tx, ty) = at(glyph.origin_x, run.ruler_y);
            window.paint_quad(fill(rect(tx, ty, 2.0, 10.0), red()));
        }
        // 字形本体：屏幕消费场景字形 id/位置，不重新整形。
        for glyph in &run.glyphs {
            let (gx, gy) = at(glyph.origin_x, glyph.origin_y);
            let origin = Point::new(px(gx as f32), px(gy as f32));
            window
                .paint_glyph(
                    origin,
                    font_id,
                    GlyphId(u32::from(glyph.glyph_id)),
                    px(run.font_size),
                    black(),
                )
                .expect("paint_glyph");
        }
    }
}

fn write_evidence(_window: &mut Window, plan: &PaintPlan, font: &ProbeFont, out: &Path) {
    let scale = plan.scale_factor;
    let font_json = [
        json::field(
            "source_path",
            &json::string(&font.source_path.display().to_string()),
            2,
        ),
        json::field("face_index", &font.face_index.to_string(), 2),
        json::field("original_sha256", &json::string(&font.original_sha256), 2),
        json::field("rewritten_sha256", &json::string(&font.rewritten_sha256), 2),
        json::field("original_family", &json::string(&font.original_family), 2),
        json::field("rewritten_family", &json::string(&font.names.family), 2),
        json::field(
            "rewritten_postscript",
            &json::string(&font.names.postscript),
            2,
        ),
        json::field(
            "license_note",
            &json::string(
                "macOS 系统内置字体（Apple 专有许可）。仅本机实验输入，不提交、不重分发。",
            ),
            2,
        ),
    ]
    .join(",\n");
    let upstream_json = [
        json::field(
            "gpui",
            &json::string("crates.io gpui 0.2.2 vendored + GlyphId(pub) visibility patch"),
            2,
        ),
        json::field(
            "gpui_registry_checksum",
            &json::string("979b45cfa6ec723b6f42330915a1b3769b930d02b2d505f9697f8ca602bee707"),
            2,
        ),
        json::field("patch", &json::string("vendor/PATCH.md"), 2),
    ]
    .join(",\n");
    let text_lines = format!(
        "[{}]",
        LINES
            .iter()
            .map(|line| json::string(line))
            .collect::<Vec<_>>()
            .join(", ")
    );
    let calibration = format!(
        "[{}]",
        CAL_POINTS
            .iter()
            .map(|(x, y)| format!("[{x}, {y}]"))
            .collect::<Vec<_>>()
            .join(", ")
    );
    let runs_json: Vec<String> = plan
        .runs
        .iter()
        .map(|run| {
            let glyphs: Vec<String> = run
                .glyphs
                .iter()
                .map(|glyph| {
                    format!(
                        "{{\n{}\n{}}}",
                        [
                            json::field("i", &glyph.index.to_string(), 3),
                            json::field("gid", &glyph.glyph_id.to_string(), 3),
                            json::field("origin_lx", &format!("{:.3}", glyph.origin_x), 3),
                            json::field("origin_ly", &format!("{:.3}", glyph.origin_y), 3),
                            json::field(
                                "tick_px",
                                &format!(
                                    "[{:.1}, {:.1}]",
                                    glyph.origin_x * scale,
                                    run.ruler_y * scale
                                ),
                                3,
                            ),
                        ]
                        .join(",\n"),
                        "  ".repeat(2)
                    )
                })
                .collect();
            format!(
                "{{\n{}\n{}}}",
                [
                    json::field("line", &run.line.to_string(), 2),
                    json::field("origin_lx", &format!("{:.3}", run.origin_x), 2),
                    json::field("baseline_ly", &format!("{:.3}", run.baseline_y), 2),
                    json::field("ruler_ly", &format!("{:.3}", run.ruler_y), 2),
                    json::field("font_size_lpx", &format!("{:.3}", run.font_size), 2),
                    json::field("glyph_count", &run.glyphs.len().to_string(), 2),
                    format!(
                        "{}\"glyphs\": [\n{}\n{}]",
                        "  ".repeat(2),
                        glyphs.join(",\n"),
                        "  ".repeat(2)
                    ),
                ]
                .join(",\n"),
                "  "
            )
        })
        .collect();
    let document = [
        "{".to_owned(),
        format!(
            "{},",
            json::field("probe", &json::string("gpui-font-probe"), 1)
        ),
        format!(
            "{},",
            json::field("upstream", &format!("{{\n{upstream_json}\n{}}}", "  "), 1)
        ),
        format!(
            "{},",
            json::field("font", &format!("{{\n{font_json}\n{}}}", "  "), 1)
        ),
        format!("{},", json::field("text_lines", &text_lines, 1)),
        format!(
            "{},",
            json::field(
                "page",
                &format!(
                    "{{\n{}\n{}}}",
                    [
                        json::field("width_emu", &plan.page_w_emu.to_string(), 2),
                        json::field("height_emu", &plan.page_h_emu.to_string(), 2),
                        json::field("origin_lx", &format!("{PAGE_X}"), 2),
                        json::field("origin_ly", &format!("{PAGE_Y}"), 2),
                    ]
                    .join(",\n"),
                    "  "
                ),
                1
            )
        ),
        format!("{},", json::field("zoom", &format!("{}", plan.zoom), 1)),
        format!("{},", json::field("scale_factor", &format!("{scale}"), 1)),
        format!(
            "{},",
            json::field("calibration_logical_px", &calibration, 1)
        ),
        json::field(
            "runs",
            &format!("[\n{}\n{}]", runs_json.join(",\n"), "  "),
            1,
        ),
        "}".to_owned(),
    ]
    .join("\n");
    let path = out.join(format!("evidence-z{:02}.json", plan.zoom * 100.0));
    std::fs::write(&path, document + "\n")
        .unwrap_or_else(|error| panic!("write {}: {error}", path.display()));
}
