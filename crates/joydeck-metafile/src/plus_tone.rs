//! Color adjustment response for MS-EMFPLUS effects on straight color channels.

pub(crate) fn contrast(normalized: f64) -> f64 {
    if normalized < 0.0 {
        1.0 + normalized
    } else {
        1.0 / (1.0 - normalized).max(0.005)
    }
}

pub(crate) fn levels(x: f64, highlight: i32, midtone: i32, shadow: i32) -> f64 {
    let black = f64::from(shadow) / 100.0;
    let white = f64::from(highlight) / 100.0;
    let window = (white - black).max(1.0 / 255.0);
    mid((x - black).clamp(0.0, window) / window, midtone)
}

fn mid(x: f64, amount: i32) -> f64 {
    // The Win32 midtone control is symmetric in gamma and its reciprocal.
    let magnitude = f64::from(amount.abs());
    const GAMMA: [f64; 6] = [2.0, 2.4, 2.8, 3.3333, 4.4, 10.0];
    let power = if magnitude <= 50.0 {
        1.0 + magnitude / 50.0
    } else {
        let index = (magnitude - 50.0) / 10.0;
        let i = (index as usize).min(GAMMA.len() - 2);
        blend(GAMMA[i], GAMMA[i + 1], index - i as f64)
    };
    x.powf(if amount >= 0 { 1.0 / power } else { power })
}

pub(crate) fn curve(x: f64, mode: u32, strength: i32) -> Option<f64> {
    Some(match mode {
        0 | 1 if (-255..=255).contains(&strength) => x + f64::from(strength) / 255.0,
        2..=4 if (-100..=100).contains(&strength) => {
            let input = x * 255.0;
            let (domain, mirror) = match mode {
                2 if input >= 128.0 => (255.0 - input, true),
                2 => (input, false),
                3 if input >= 128.0 => (255.0 - input, true),
                4 if input < 128.0 => (input, false),
                _ => return Some(x),
            };
            let value = shaped_curve(domain, mode, strength);
            (if mirror { 255.0 - value } else { value }) / 255.0
        }
        5 if (-100..=100).contains(&strength) => smooth(x, 20, |x| mid(x, strength)),
        6 if (1..=255).contains(&strength) => {
            smooth(x, 22, |x| (x * 255.0 / f64::from(strength)).min(1.0))
        }
        7 if (0..=254).contains(&strength) => {
            let black = f64::from(strength) / 255.0;
            (x - black) / (1.0 - black)
        }
        _ => return None,
    })
}

fn smooth(x: f64, segments: usize, curve: impl Fn(f64) -> f64) -> f64 {
    let step = segments as f64;
    let scaled = x * step;
    let i = (scaled as usize).min(segments - 1);
    let t = scaled - i as f64;
    let value = |index: usize| curve(index as f64 / step);
    let (a, b) = (value(i), value(i + 1));
    let delta = b - a;
    let left = if i == 0 {
        (3.0 * delta - (value(2) - b)) * 0.5
    } else {
        (b - value(i - 1)) * 0.5
    };
    let right = if i + 2 > segments {
        (3.0 * delta - (a - value(i - 1))) * 0.5
    } else {
        (value(i + 2) - a) * 0.5
    };
    let slope = t * (1.0 - t);
    blend(a, b, t * t * (3.0 - 2.0 * t)) + slope * ((1.0 - t) * left - t * right)
}

fn shaped_curve(input: f64, mode: u32, amount: i32) -> f64 {
    if amount == 0 {
        return input;
    }
    let controls = &CURVES[mode as usize - 2];
    let row = (f64::from(amount) + 100.0) / 25.0;
    let lower = (row as usize).min(controls.len() - 2);
    let interval = (input / 8.0) as usize;
    let sample = |r: usize| {
        let prev = f64::from(controls[r][interval]);
        let next = f64::from(controls[r][interval + 1]);
        let t = input / 8.0 - interval as f64;
        blend(prev, next, t)
    };
    blend(sample(lower), sample(lower + 1), row - lower as f64)
}

fn blend(a: f64, b: f64, t: f64) -> f64 {
    (b - a).mul_add(t, a)
}

// Half-ramp control points measured with Win32 ApplyEffect at 25-point control intervals.
// Contrast mirrors its lower half. Highlight uses the distance from white.
const CURVES: [[[u8; 17]; 9]; 3] = [
    [
        [
            0, 68, 101, 105, 111, 117, 120, 122, 124, 125, 126, 127, 127, 127, 128, 128, 128,
        ],
        [
            0, 43, 71, 83, 93, 100, 106, 110, 114, 117, 119, 121, 123, 124, 125, 126, 128,
        ],
        [
            0, 24, 43, 59, 71, 81, 89, 95, 101, 105, 109, 113, 117, 119, 122, 125, 128,
        ],
        [
            0, 13, 26, 38, 49, 59, 68, 76, 84, 91, 97, 103, 108, 113, 118, 123, 128,
        ],
        [
            0, 8, 16, 24, 32, 40, 48, 56, 64, 72, 80, 88, 96, 104, 112, 120, 128,
        ],
        [
            0, 5, 10, 15, 20, 25, 31, 37, 44, 52, 60, 69, 79, 90, 102, 115, 128,
        ],
        [
            0, 3, 5, 8, 11, 14, 18, 22, 27, 33, 39, 47, 57, 70, 85, 105, 129,
        ],
        [0, 1, 2, 4, 5, 7, 9, 11, 14, 17, 22, 27, 35, 45, 58, 86, 130],
        [0, 0, 0, 0, 0, 1, 1, 2, 4, 5, 8, 11, 17, 23, 27, 63, 132],
    ],
    [
        [
            0, 61, 91, 95, 99, 102, 104, 105, 106, 106, 107, 109, 111, 114, 118, 122, 128,
        ],
        [
            0, 39, 64, 76, 85, 91, 95, 97, 99, 101, 103, 105, 108, 112, 117, 122, 128,
        ],
        [
            0, 22, 41, 55, 67, 75, 82, 87, 91, 94, 97, 101, 105, 110, 115, 121, 128,
        ],
        [
            0, 13, 25, 37, 48, 57, 66, 73, 79, 85, 90, 96, 101, 107, 114, 121, 128,
        ],
        [
            0, 8, 16, 24, 32, 40, 48, 56, 64, 72, 80, 88, 96, 104, 112, 120, 128,
        ],
        [
            0, 5, 10, 15, 20, 26, 32, 39, 46, 55, 65, 77, 89, 100, 110, 119, 128,
        ],
        [
            0, 3, 6, 9, 12, 15, 20, 24, 30, 37, 45, 59, 76, 94, 107, 118, 128,
        ],
        [
            0, 1, 3, 4, 6, 8, 10, 13, 16, 21, 27, 34, 54, 84, 104, 117, 128,
        ],
        [0, 0, 0, 1, 1, 2, 3, 4, 6, 9, 12, 16, 27, 52, 91, 118, 128],
    ],
    [
        [0, 0, 0, 1, 1, 2, 3, 4, 6, 9, 12, 17, 24, 32, 49, 88, 129],
        [
            0, 1, 3, 4, 6, 8, 10, 13, 16, 21, 27, 34, 42, 54, 81, 110, 128,
        ],
        [
            0, 3, 6, 9, 12, 15, 20, 24, 30, 37, 44, 52, 64, 79, 97, 115, 128,
        ],
        [
            0, 5, 10, 15, 20, 26, 32, 39, 46, 54, 62, 72, 83, 95, 106, 118, 128,
        ],
        [
            0, 8, 16, 24, 32, 40, 48, 56, 64, 72, 80, 88, 96, 104, 112, 120, 128,
        ],
        [
            0, 13, 25, 37, 48, 57, 66, 74, 82, 88, 94, 99, 105, 110, 116, 122, 128,
        ],
        [
            0, 22, 41, 55, 67, 76, 84, 91, 96, 101, 104, 108, 111, 115, 119, 123, 128,
        ],
        [
            0, 39, 64, 76, 86, 94, 100, 105, 108, 110, 112, 114, 116, 119, 122, 124, 128,
        ],
        [
            0, 61, 91, 96, 104, 110, 113, 115, 116, 117, 118, 118, 120, 122, 124, 125, 128,
        ],
    ],
];
