//! Time easing: acceleration / deceleration fractions and bounce.

use crate::clampf;

/// Map linear progress `p` (0..1) through PowerPoint-style easing: `accel` and `decel` are the
/// fractions of the duration spent speeding up and slowing down (velocity ramps linearly);
/// `bounce` (0..1) makes the last part of the motion bounce against the end.
pub fn ease(p: f64, accel: f64, decel: f64, bounce: f64) -> f64 {
    let p = clampf(p, 0.0, 1.0, 0.0);
    let bounce = clampf(bounce, 0.0, 1.0, 0.0);
    let position = if bounce > 0.0 {
        let impact = 1.0 - bounce;
        if p < impact {
            let approach = p / impact;
            approach * approach
        } else {
            1.0 - bounce_hops((p - impact) / bounce) * 0.25
        }
    } else {
        let rise = clampf(accel, 0.0, 1.0, 0.0);
        let fall = clampf(decel, 0.0, 1.0, 0.0);
        let sum = (rise + fall).max(1.0);
        ramp_position(p, rise / sum, fall / sum)
    };
    clampf(position, -1.0, 2.0, 0.0)
}

fn ramp_position(p: f64, rise: f64, fall: f64) -> f64 {
    // Integrate the trapezoidal velocity curve. Its area over one iteration is one.
    let speed = 2.0 / (2.0 - rise - fall);
    if p < rise {
        speed * p * p / (2.0 * rise)
    } else if p > 1.0 - fall {
        let remaining = 1.0 - p;
        1.0 - speed * remaining * remaining / (2.0 * fall)
    } else {
        speed * rise / 2.0 + speed * (p - rise)
    }
}

/// Decaying hops for s in 0..1: 0 at the ends of each hop, peaks shrinking.
fn bounce_hops(s: f64) -> f64 {
    let s = clampf(s, 0.0, 1.0, 1.0);
    let hops = 3.0;
    let k = (s * hops).floor().min(hops - 1.0);
    let local = s * hops - k;
    let amp = 0.5f64.powf(k);
    amp * (local * std::f64::consts::PI).sin()
}

/// Classic bounce-out: falls to 1 and bounces with decaying height.
pub(crate) fn bounce_out(p: f64) -> f64 {
    let p = clampf(p, 0.0, 1.0, 0.0);
    let n = 7.5625;
    let d = 2.75;
    let y = if p < 1.0 / d {
        n * p * p
    } else if p < 2.0 / d {
        let q = p - 1.5 / d;
        n * q * q + 0.75
    } else if p < 2.5 / d {
        let q = p - 2.25 / d;
        n * q * q + 0.9375
    } else {
        let q = p - 2.625 / d;
        n * q * q + 0.984375
    };
    y.min(1.0)
}

/// Smooth ease-in-out (cubic smoothstep).
pub(crate) fn smooth(p: f64) -> f64 {
    let p = clampf(p, 0.0, 1.0, 0.0);
    p * p * (3.0 - 2.0 * p)
}
