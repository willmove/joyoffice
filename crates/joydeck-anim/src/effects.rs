//! Evaluation of one animation effect at a progress value.

use std::f64::consts::PI;

use joydeck_model::{AnimClass, Animation, ColorRef, Rgba, Xfrm};

use crate::AnimState;
use crate::easing::{bounce_out, ease, smooth};
use crate::path::{MotionPath, default_path};

/// Everything an effect needs besides its progress.
#[derive(Clone, Debug)]
pub(crate) struct EffectSpec {
    kind: EffectKind,
    option: String,
    timing: Curve,
    amount: Option<f64>,
    color: Option<Rgba>,
    /// Shape box in points (sanitized).
    xf: Xfrm,
    slide_w: f64,
    slide_h: f64,
}

#[derive(Clone, Debug)]
enum EffectKind {
    Reveal(Reveal),
    Conceal(Reveal),
    Emphasis(Emphasis),
    Travel(MotionPath),
    Media,
}

#[derive(Clone, Copy, Debug)]
enum Reveal {
    Cut,
    Opacity,
    Fly,
    Float,
    Split,
    Wipe,
    Iris { rounded: bool },
    Grow,
    Zoom,
    Swivel,
    Bounce,
    Peek,
    Strips,
    Expand,
    Spinner,
}

impl Reveal {
    fn for_id(id: &str, option: &str) -> Self {
        match id {
            "appear" => Self::Cut,
            "fly" => Self::Fly,
            "float" | "rise" => Self::Float,
            "split" => Self::Split,
            "wipe" => Self::Wipe,
            "shape" | "circle" | "box" | "diamond" | "expandClip" => Self::Iris {
                rounded: matches!(id, "circle" | "diamond")
                    || ["circle", "diamond", "plus"]
                        .iter()
                        .any(|name| option.starts_with(name)),
            },
            "grow" => Self::Grow,
            "zoom" => Self::Zoom,
            "swivel" => Self::Swivel,
            "bounce" => Self::Bounce,
            "peek" => Self::Peek,
            "strips" => Self::Strips,
            "expand" => Self::Expand,
            "spinner" => Self::Spinner,
            _ => Self::Opacity,
        }
    }
}

#[derive(Clone, Copy, Debug)]
enum Emphasis {
    None,
    Pulse,
    ColorPulse,
    Teeter,
    Spin,
    Resize,
    Desaturate,
    Darken,
    Lighten,
    Transparency,
    Color,
    Complement,
    Text { flash: bool },
    Wave,
}

impl Emphasis {
    fn for_id(id: &str) -> Self {
        match id {
            "pulse" => Self::Pulse,
            "colorPulse" => Self::ColorPulse,
            "teeter" => Self::Teeter,
            "spin" => Self::Spin,
            "growShrink" => Self::Resize,
            "desaturate" => Self::Desaturate,
            "darken" => Self::Darken,
            "lighten" => Self::Lighten,
            "transparency" => Self::Transparency,
            "objectColor" | "fillColor" | "lineColor" | "fontColor" => Self::Color,
            "complementaryColor" => Self::Complement,
            "underline" | "boldFlash" => Self::Text {
                flash: id == "boldFlash",
            },
            "wave" => Self::Wave,
            _ => Self::None,
        }
    }

    fn persists(self) -> bool {
        matches!(
            self,
            Self::Spin
                | Self::Resize
                | Self::Desaturate
                | Self::Darken
                | Self::Lighten
                | Self::Transparency
                | Self::Color
                | Self::Complement
        )
    }
}

#[derive(Clone, Debug)]
enum Curve {
    Smooth,
    Timed { accel: f64, decel: f64, bounce: f64 },
}

/// Map an exit effect id to the entrance effect it mirrors.
fn exit_base(effect: &str) -> &str {
    match effect {
        "disappear" => "appear",
        "fadeOut" => "fade",
        "flyOut" => "fly",
        "floatOut" => "float",
        "splitOut" => "split",
        "wipeOut" => "wipe",
        "shapeOut" => "shape",
        "wheelOut" => "wheel",
        "randomBarsOut" => "randomBars",
        "shrinkTurn" => "grow",
        "zoomOut" => "zoom",
        "swivelOut" => "swivel",
        "bounceOut" => "bounce",
        "dissolveOut" => "dissolve",
        "blindsOut" => "blinds",
        "boxOut" => "box",
        "checkerboardOut" => "checkerboard",
        "diamondOut" => "diamond",
        "peekOut" => "peek",
        "stripsOut" => "strips",
        "sinkDown" => "rise",
        "spinnerOut" => "spinner",
        "contract" => "expand",
        other => other,
    }
}

fn uses_default_smoothing(effect: &str) -> bool {
    matches!(
        effect,
        "fly"
            | "float"
            | "rise"
            | "peek"
            | "lines"
            | "arcs"
            | "turns"
            | "shapes"
            | "loops"
            | "customPath"
    ) || joydeck_model::anim::preset_path(effect).is_some()
}

impl EffectSpec {
    pub(crate) fn compile(
        a: &Animation,
        xf: Xfrm,
        slide_w: f64,
        slide_h: f64,
        resolve: &dyn Fn(&ColorRef) -> Rgba,
    ) -> Self {
        let effect = if a.class == AnimClass::Exit {
            exit_base(&a.effect)
        } else {
            &a.effect
        };
        let option = if a.effect == "sinkDown" {
            "d"
        } else {
            &a.option
        };
        let kind = match a.class {
            AnimClass::Entrance => EffectKind::Reveal(Reveal::for_id(effect, option)),
            AnimClass::Exit => EffectKind::Conceal(Reveal::for_id(effect, option)),
            AnimClass::Emphasis => EffectKind::Emphasis(Emphasis::for_id(effect)),
            AnimClass::Path => {
                let preset;
                let path = match a.path.as_deref().filter(|s| !s.trim().is_empty()) {
                    Some(path) => path,
                    None => {
                        preset = default_path(&a.effect, &a.option);
                        &preset
                    }
                };
                EffectKind::Travel(MotionPath::parse(path))
            }
            AnimClass::Media => EffectKind::Media,
        };
        let timing = if a.smooth_start <= 0.0
            && a.smooth_end <= 0.0
            && a.bounce_end <= 0.0
            && uses_default_smoothing(effect)
        {
            Curve::Smooth
        } else {
            Curve::Timed {
                accel: a.smooth_start,
                decel: a.smooth_end,
                bounce: a.bounce_end,
            }
        };
        Self {
            kind,
            option: option.into(),
            timing,
            amount: a.amount,
            color: a.color.as_ref().map(resolve),
            xf,
            slide_w,
            slide_h,
        }
    }

    fn eased(&self, p: f64) -> f64 {
        match self.timing {
            Curve::Smooth => smooth(p),
            Curve::Timed {
                accel,
                decel,
                bounce,
            } => ease(p, accel, decel, bounce),
        }
    }

    /// The state delta at linear progress `p` (0..1) while the effect plays. `cur` is the shape's
    /// current fill colour (for colour emphasis).
    pub fn eval(&self, p: f64, cur: Rgba) -> AnimState {
        let p = crate::clampf(p, 0.0, 1.0, 0.0);
        let s = match &self.kind {
            EffectKind::Reveal(reveal) => self.reveal(*reveal, p, false),
            EffectKind::Conceal(reveal) => self.reveal(*reveal, 1.0 - p, true),
            EffectKind::Emphasis(emphasis) => self.emphasis(*emphasis, p, cur),
            EffectKind::Travel(path) => self.motion(path, p),
            EffectKind::Media => AnimState::default(),
        };
        s.sanitized()
    }

    /// The state the effect leaves behind once done.
    pub fn end_state(&self, cur: Rgba) -> AnimState {
        match &self.kind {
            EffectKind::Conceal(_) => AnimState::hidden(),
            EffectKind::Emphasis(kind) if kind.persists() => {
                self.emphasis(*kind, 1.0, cur).sanitized()
            }
            EffectKind::Travel(path) => self.motion(path, 1.0).sanitized(),
            _ => AnimState::default(),
        }
    }

    fn reveal(&self, effect: Reveal, p: f64, exit: bool) -> AnimState {
        let e = self.eased(p);
        let q = 1.0 - e;
        let o = self.option.as_str();
        let Xfrm { x, y, w, h, .. } = self.xf;
        let (sw, sh) = (self.slide_w, self.slide_h);
        let mut st = AnimState::default();
        let center_clip = |f: f64, horz: bool, vert: bool| {
            let f = f.clamp(0.0, 1.0) / 2.0;
            let (l, r) = if horz { (0.5 - f, 0.5 + f) } else { (0.0, 1.0) };
            let (t, b) = if vert { (0.5 - f, 0.5 + f) } else { (0.0, 1.0) };
            Some([l, t, r, b])
        };
        match effect {
            Reveal::Cut => st.visible = p > 0.0,
            Reveal::Opacity => st.opacity = e,
            Reveal::Fly => {
                let o = if o.is_empty() { "b" } else { o };
                st.offset_x = travel_from(o, 'l', 'r', x, w, sw, q);
                st.offset_y = travel_from(o, 't', 'b', y, h, sh, q);
            }
            Reveal::Float => {
                // Float Up enters from below and leaves upward; Float Down the reverse.
                let down = o == "d";
                let dir = if down { -1.0 } else { 1.0 };
                let dir = if exit { -dir } else { dir };
                st.offset_y = dir * 0.1 * sh * q;
                st.opacity = e;
            }
            Reveal::Split => {
                let horz = o.starts_with("horz");
                // "In" closes from the edges: approximated with the centre reveal plus a fade.
                st.clip = if horz {
                    center_clip(e, false, true)
                } else {
                    center_clip(e, true, false)
                };
                if o.ends_with("In") {
                    st.opacity = e;
                }
            }
            Reveal::Wipe => {
                st.clip = Some(match o {
                    "l" => [0.0, 0.0, e, 1.0],
                    "t" => [0.0, 0.0, 1.0, e],
                    "r" => [1.0 - e, 0.0, 1.0, 1.0],
                    _ => [0.0, 1.0 - e, 1.0, 1.0],
                });
            }
            Reveal::Iris { rounded } => {
                let f = if rounded {
                    (e * std::f64::consts::SQRT_2).min(1.0)
                } else {
                    e
                };
                st.clip = center_clip(f, true, true);
                if o.ends_with("In") || o == "in" {
                    st.opacity = e;
                }
            }
            Reveal::Grow => {
                st.scale_x = e;
                st.scale_y = e;
                st.rotate = -90.0 * q;
                st.opacity = e;
            }
            Reveal::Zoom => {
                let s = if o == "out" { 1.0 + 3.0 * q } else { e };
                st.scale_x = s;
                st.scale_y = s;
                st.opacity = e;
            }
            Reveal::Swivel => {
                let s = e * (q * 2.5 * PI).cos();
                if o == "vert" {
                    st.scale_y = s;
                } else {
                    st.scale_x = s;
                }
                st.opacity = e.min(1.0);
            }
            Reveal::Bounce => {
                let b = bounce_out(p);
                st.offset_y = -(0.25 * sh) * (1.0 - b);
                st.offset_x = -0.1 * sw * (1.0 - p);
                st.opacity = (p * 4.0).min(1.0);
            }
            Reveal::Peek => match o {
                "l" => {
                    st.offset_x = -w * q;
                    st.clip = Some([q, 0.0, 1.0, 1.0]);
                }
                "r" => {
                    st.offset_x = w * q;
                    st.clip = Some([0.0, 0.0, e, 1.0]);
                }
                "t" => {
                    st.offset_y = -h * q;
                    st.clip = Some([0.0, q, 1.0, 1.0]);
                }
                _ => {
                    st.offset_y = h * q;
                    st.clip = Some([0.0, 0.0, 1.0, e]);
                }
            },
            Reveal::Strips => {
                st.clip = Some(match o {
                    "ru" => [0.0, q, e, 1.0],
                    "ld" => [q, 0.0, 1.0, e],
                    "rd" => [0.0, 0.0, e, e],
                    _ => [q, q, 1.0, 1.0],
                });
            }
            Reveal::Expand => {
                st.scale_x = 0.3 + 0.7 * e;
                st.opacity = e;
            }
            Reveal::Spinner => {
                st.rotate = 360.0 * q;
                st.scale_x = 0.5 + 0.5 * e;
                st.scale_y = 0.5 + 0.5 * e;
                st.opacity = e;
            }
        }
        st
    }

    fn emphasis(&self, effect: Emphasis, p: f64, cur: Rgba) -> AnimState {
        let e = self.eased(p);
        let bump = (p * PI).sin();
        let mut st = AnimState::default();
        let target = self.color.unwrap_or(Rgba::rgb(0xC0, 0x00, 0x00));
        match effect {
            Emphasis::Pulse => {
                st.scale_x = 1.0 + 0.1 * bump;
                st.scale_y = st.scale_x;
            }
            Emphasis::ColorPulse => st.tint = Some(cur.lerp(target, bump)),
            Emphasis::Teeter => st.rotate = 8.0 * (p * 4.0 * PI).sin() * (1.0 - p * 0.5),
            Emphasis::Spin => {
                let amt = self
                    .amount
                    .filter(|a| a.is_finite())
                    .unwrap_or(360.0)
                    .clamp(-36_000.0, 36_000.0);
                let sign = if self.option == "counterClockwise" {
                    -1.0
                } else {
                    1.0
                };
                st.rotate = sign * amt * e;
            }
            Emphasis::Resize => {
                let amt = self
                    .amount
                    .filter(|a| a.is_finite() && *a > 0.0)
                    .unwrap_or(1.5)
                    .clamp(0.0, 100.0);
                let s = 1.0 + (amt - 1.0) * e;
                match self.option.as_str() {
                    "horz" => st.scale_x = s,
                    "vert" => st.scale_y = s,
                    _ => {
                        st.scale_x = s;
                        st.scale_y = s;
                    }
                }
            }
            Emphasis::Desaturate => {
                let l = (0.299 * cur.r as f64 + 0.587 * cur.g as f64 + 0.114 * cur.b as f64)
                    .round()
                    .clamp(0.0, 255.0) as u8;
                st.tint = Some(cur.lerp(Rgba::rgba(l, l, l, cur.a), e));
            }
            Emphasis::Darken => st.tint = Some(cur.lerp(Rgba::rgba(0, 0, 0, cur.a), 0.4 * e)),
            Emphasis::Lighten => {
                st.tint = Some(cur.lerp(Rgba::rgba(255, 255, 255, cur.a), 0.4 * e))
            }
            Emphasis::Transparency => {
                // `amount` is the transparency (0.5 = 50 %).
                let a = self
                    .amount
                    .filter(|a| a.is_finite())
                    .unwrap_or(0.5)
                    .clamp(0.0, 1.0);
                st.opacity = 1.0 - a * e;
            }
            Emphasis::Color => st.tint = Some(cur.lerp(target, e)),
            Emphasis::Complement => {
                st.tint =
                    Some(cur.lerp(Rgba::rgba(255 - cur.r, 255 - cur.g, 255 - cur.b, cur.a), e))
            }
            Emphasis::Text { flash } => {
                st.scale_x = 1.0 + 0.04 * bump;
                st.scale_y = st.scale_x;
                if flash {
                    st.tint = Some(cur.lerp(Rgba::rgba(0, 0, 0, cur.a), 0.3 * bump));
                }
            }
            Emphasis::Wave => {
                st.offset_y = -0.02 * self.slide_h * bump;
                st.scale_y = 1.0 + 0.05 * bump;
            }
            Emphasis::None => {}
        }
        st
    }

    fn motion(&self, path: &MotionPath, p: f64) -> AnimState {
        let (fx, fy) = path.at(self.eased(p));
        AnimState {
            offset_x: fx * self.slide_w,
            offset_y: fy * self.slide_h,
            ..Default::default()
        }
    }
}

fn travel_from(
    option: &str,
    near: char,
    far: char,
    start: f64,
    size: f64,
    limit: f64,
    remaining: f64,
) -> f64 {
    if option.contains(far) {
        (limit - start) * remaining
    } else if option.contains(near) {
        -(start + size) * remaining
    } else {
        0.0
    }
}
