//! Device context commands carried by EMF+ records.

use crate::{bytes::Bytes, device::Xform, geom::P};

pub(crate) enum State {
    Save(u32),
    Restore(u32),
    Transform(Xform),
    Concat(Xform),
    Page(f64),
    ClearClip,
    ClipRectangle([f64; 4]),
    ClipPath(usize),
    ClipRegion(usize),
    ClipOffset(P),
}

fn coordinates<const N: usize>(data: Bytes<'_>) -> Option<[f64; N]> {
    let mut values = [0.0; N];
    for (index, value) in values.iter_mut().enumerate() {
        *value = f64::from(data.f32(index * 4)?);
        if !value.is_finite() {
            return None;
        }
    }
    Some(values)
}

pub(crate) fn state(kind: u16, flags: u16, data: Bytes<'_>) -> Option<State> {
    let object = usize::from(flags & 0xff);
    Some(match kind {
        0x4025 | 0x4028 => State::Save(data.u32(0)?),
        0x4027 => State::Save(data.u32(24)?),
        0x4026 | 0x4029 => State::Restore(data.u32(0)?),
        0x402a => State::Transform(coordinates(data)?),
        0x402b => State::Transform([1.0, 0.0, 0.0, 1.0, 0.0, 0.0]),
        0x402c => State::Concat(coordinates(data)?),
        0x402d => {
            let [x, y] = coordinates(data)?;
            State::Concat([1.0, 0.0, 0.0, 1.0, x, y])
        }
        0x402e => {
            let [x, y] = coordinates(data)?;
            State::Concat([x, 0.0, 0.0, y, 0.0, 0.0])
        }
        0x402f => {
            let [angle] = coordinates(data)?;
            let (s, c) = angle.to_radians().sin_cos();
            State::Concat([c, s, -s, c, 0.0, 0.0])
        }
        0x4030 => {
            let [scale] = coordinates(data)?;
            State::Page(scale)
        }
        0x4031 => State::ClearClip,
        0x4032 => {
            let [x, y, w, h] = coordinates(data)?;
            State::ClipRectangle([x, y, x + w, y + h])
        }
        0x4033 => State::ClipPath(object),
        0x4034 => State::ClipRegion(object),
        0x4035 => {
            let [x, y] = coordinates(data)?;
            State::ClipOffset((x, y))
        }
        _ => return None,
    })
}
