//! CONVERT unit catalogue and dimensional conversion.
//!
//! Symbols and prefixes follow Microsoft's CONVERT documentation. The catalogue
//! also retains historical JoySheet aliases and conversion constants so existing
//! workbooks recalculate consistently.

use std::borrow::Cow;

use super::*;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Quantity {
    Area,
    Energy,
    Force,
    Information,
    Length,
    Magnetism,
    Mass,
    Power,
    Pressure,
    Speed,
    Temperature,
    Time,
    Volume,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Temperature {
    Kelvin,
    Celsius,
    Fahrenheit,
    Rankine,
    Reaumur,
}

impl Temperature {
    fn kelvin(self, value: f64) -> f64 {
        match self {
            Self::Kelvin => value,
            Self::Celsius => value + 273.15,
            Self::Fahrenheit => (value - 32.0) * 5.0 / 9.0 + 273.15,
            Self::Rankine => value * 5.0 / 9.0,
            Self::Reaumur => value * 1.25 + 273.15,
        }
    }

    fn reading(self, kelvin: f64) -> f64 {
        match self {
            Self::Kelvin => kelvin,
            Self::Celsius => kelvin - 273.15,
            Self::Fahrenheit => (kelvin - 273.15) * 9.0 / 5.0 + 32.0,
            Self::Rankine => kelvin * 9.0 / 5.0,
            Self::Reaumur => (kelvin - 273.15) / 1.25,
        }
    }
}

struct Unit {
    quantity: Quantity,
    scale: f64,
    metric_power: i32,
    temperature: Option<Temperature>,
}

impl Unit {
    fn linear(quantity: Quantity, scale: f64, metric_power: i32) -> Self {
        Self { quantity, scale, metric_power, temperature: None }
    }

    fn thermal(temperature: Temperature) -> Self {
        Self {
            quantity: Quantity::Temperature,
            scale: 1.0,
            metric_power: i32::from(temperature == Temperature::Kelvin),
            temperature: Some(temperature),
        }
    }
}

const INCH: f64 = 0.0254;
const FOOT: f64 = 0.3048;
const YARD: f64 = 0.9144;
const MILE: f64 = 1609.344;
const NAUTICAL_MILE: f64 = 1852.0;
const LIGHT_YEAR: f64 = 9_460_730_472_580_800.0;
const POINT: f64 = INCH / 72.0;
const PICA: f64 = INCH / 6.0;
const US_GALLON: f64 = 0.003_785_411_784;

fn exact(symbol: &str) -> Option<Unit> {
    use Quantity::*;
    let unit = match symbol {
        "admkn" => Unit::linear(Speed, 6080.0 * FOOT / 3600.0, 0),
        "ang" => Unit::linear(Length, 1e-10, 1),
        "ang2" => Unit::linear(Area, 1e-20, 2),
        "ang3" => Unit::linear(Volume, 1e-30, 3),
        "ar" => Unit::linear(Area, 100.0, 1),
        "at" | "atm" => Unit::linear(Pressure, 101_325.0, 1),
        "barrel" => Unit::linear(Volume, 0.158_987_294_928, 0),
        "bit" => Unit::linear(Information, 1.0, 1),
        "BTU" | "btu" => Unit::linear(Energy, 1_055.055_852_62, 0),
        "bushel" => Unit::linear(Volume, 0.035_239_070_166_88, 0),
        "byte" => Unit::linear(Information, 8.0, 1),
        "c" => Unit::linear(Energy, 4.184, 1),
        "cal" => Unit::linear(Energy, 4.1868, 1),
        "cup" => Unit::linear(Volume, US_GALLON / 16.0, 0),
        "cwt" | "shweight" => Unit::linear(Mass, 45_359.237, 0),
        "d" | "day" => Unit::linear(Time, 86_400.0, 0),
        "dy" | "dyn" => Unit::linear(Force, 1e-5, 1),
        "e" => Unit::linear(Energy, 1e-7, 1),
        "ell" => Unit::linear(Length, 1.143, 0),
        "eV" | "ev" => Unit::linear(Energy, 1.602_176_634e-19, 1),
        "flb" => Unit::linear(Energy, 1.355_817_948_331_4, 0),
        "ft" => Unit::linear(Length, FOOT, 0),
        "ft2" => Unit::linear(Area, FOOT * FOOT, 0),
        "ft3" => Unit::linear(Volume, FOOT * FOOT * FOOT, 0),
        "g" => Unit::linear(Mass, 1.0, 1),
        "ga" => Unit::linear(Magnetism, 1e-4, 1),
        "gal" => Unit::linear(Volume, US_GALLON, 0),
        "grain" => Unit::linear(Mass, 0.064_798_91, 0),
        "GRT" | "regton" => Unit::linear(Volume, 2.831_684_659_2, 0),
        "ha" => Unit::linear(Area, 10_000.0, 0),
        "HP" | "h" => Unit::linear(Power, 745.699_871_582_270_2, 0),
        "HPh" | "hh" => Unit::linear(Energy, 2_684_519.537_696_17, 0),
        "hr" => Unit::linear(Time, 3_600.0, 0),
        "hweight" | "lcwt" | "uk_cwt" => Unit::linear(Mass, 50_802.345_44, 0),
        "in" => Unit::linear(Length, INCH, 0),
        "in2" => Unit::linear(Area, INCH * INCH, 0),
        "in3" => Unit::linear(Volume, INCH * INCH * INCH, 0),
        "J" => Unit::linear(Energy, 1.0, 1),
        "kn" => Unit::linear(Speed, NAUTICAL_MILE / 3600.0, 0),
        "L" | "l" | "lt" => Unit::linear(Volume, 0.001, 1),
        "lbf" => Unit::linear(Force, 4.448_221_615_260_5, 0),
        "lbm" => Unit::linear(Mass, 453.592_37, 0),
        "LTON" | "brton" | "uk_ton" => Unit::linear(Mass, 1_016_046.908_8, 0),
        "ly" => Unit::linear(Length, LIGHT_YEAR, 1),
        "ly2" => Unit::linear(Area, LIGHT_YEAR * LIGHT_YEAR, 0),
        "ly3" => Unit::linear(Volume, LIGHT_YEAR * LIGHT_YEAR * LIGHT_YEAR, 0),
        "m" => Unit::linear(Length, 1.0, 1),
        "m/h" | "m/hr" => Unit::linear(Speed, 1.0 / 3600.0, 1),
        "m/s" | "m/sec" => Unit::linear(Speed, 1.0, 1),
        "m2" => Unit::linear(Area, 1.0, 2),
        "m3" => Unit::linear(Volume, 1.0, 3),
        "mi" => Unit::linear(Length, MILE, 0),
        "mi2" => Unit::linear(Area, MILE * MILE, 0),
        "mi3" => Unit::linear(Volume, MILE * MILE * MILE, 0),
        "min" | "mn" => Unit::linear(Time, 60.0, 0),
        "mmHg" => Unit::linear(Pressure, 133.322_387_415, 1),
        "Morgen" => Unit::linear(Area, 2_500.0, 0),
        "mph" => Unit::linear(Speed, MILE / 3600.0, 0),
        "MTON" => Unit::linear(Volume, 1.132_673_863_68, 0),
        "N" => Unit::linear(Force, 1.0, 1),
        "Nmi" => Unit::linear(Length, NAUTICAL_MILE, 0),
        "Nmi2" => Unit::linear(Area, NAUTICAL_MILE * NAUTICAL_MILE, 0),
        "Nmi3" => Unit::linear(Volume, NAUTICAL_MILE * NAUTICAL_MILE * NAUTICAL_MILE, 0),
        "oz" => Unit::linear(Volume, US_GALLON / 128.0, 0),
        "ozm" => Unit::linear(Mass, 28.349_523_125, 0),
        "Pa" | "p" => Unit::linear(Pressure, 1.0, 1),
        "parsec" | "pc" => Unit::linear(Length, 30_856_775_814_913_673.0, 1),
        "pica" => Unit::linear(Length, PICA, 0),
        "Pica" | "Picapt" => Unit::linear(Length, POINT, 0),
        "Pica2" | "Picapt2" => Unit::linear(Area, POINT * POINT, 0),
        "Pica3" | "Picapt3" => Unit::linear(Volume, POINT * POINT * POINT, 0),
        "pond" => Unit::linear(Force, 0.009_806_65, 1),
        "PS" => Unit::linear(Power, 735.498_75, 0),
        "psi" => Unit::linear(Pressure, 6_894.757_293_168_36, 0),
        "pt" | "us_pt" => Unit::linear(Volume, US_GALLON / 8.0, 0),
        "qt" => Unit::linear(Volume, US_GALLON / 4.0, 0),
        "s" | "sec" => Unit::linear(Time, 1.0, 1),
        "sg" => Unit::linear(Mass, 14_593.902_937_206_4, 0),
        "stone" => Unit::linear(Mass, 6_350.293_18, 0),
        "survey_mi" => Unit::linear(Length, 1_609.347_218_694_437, 0),
        "T" => Unit::linear(Magnetism, 1.0, 1),
        "tbs" => Unit::linear(Volume, US_GALLON / 256.0, 0),
        "ton" => Unit::linear(Mass, 907_184.74, 0),
        "Torr" => Unit::linear(Pressure, 101_325.0 / 760.0, 0),
        "tsp" => Unit::linear(Volume, US_GALLON / 768.0, 0),
        "tspm" => Unit::linear(Volume, 5e-6, 0),
        "u" => Unit::linear(Mass, 1.660_539_066_6e-24, 1),
        "uk_acre" => Unit::linear(Area, 4_046.856_422_4, 0),
        "uk_gal" => Unit::linear(Volume, 0.004_546_09, 0),
        "uk_pt" => Unit::linear(Volume, 0.000_568_261_25, 0),
        "uk_qt" => Unit::linear(Volume, 0.001_136_522_5, 0),
        "us_acre" => Unit::linear(Area, 4_046.872_609_874_252, 0),
        "W" | "w" => Unit::linear(Power, 1.0, 1),
        "Wh" | "wh" => Unit::linear(Energy, 3_600.0, 1),
        "yd" => Unit::linear(Length, YARD, 0),
        "yd2" => Unit::linear(Area, YARD * YARD, 0),
        "yd3" => Unit::linear(Volume, YARD * YARD * YARD, 0),
        "yr" => Unit::linear(Time, 31_557_600.0, 0),
        "cel" | "C" => Unit::thermal(self::Temperature::Celsius),
        "fah" | "F" => Unit::thermal(self::Temperature::Fahrenheit),
        "kel" | "K" => Unit::thermal(self::Temperature::Kelvin),
        "Rank" => Unit::thermal(self::Temperature::Rankine),
        "Reau" => Unit::thermal(self::Temperature::Reaumur),
        _ => return None,
    };
    Some(unit)
}

fn metric_factor(prefix: &str) -> Option<f64> {
    Some(match prefix {
        "Y" => 1e24,
        "Z" => 1e21,
        "E" => 1e18,
        "P" => 1e15,
        "T" => 1e12,
        "G" => 1e9,
        "M" => 1e6,
        "k" => 1e3,
        "h" => 1e2,
        "da" | "e" => 1e1,
        "d" => 1e-1,
        "c" => 1e-2,
        "m" => 1e-3,
        "u" => 1e-6,
        "n" => 1e-9,
        "p" => 1e-12,
        "f" => 1e-15,
        "a" => 1e-18,
        "z" => 1e-21,
        "y" => 1e-24,
        _ => return None,
    })
}

fn binary_exponent(prefix: &str) -> Option<i32> {
    Some(match prefix {
        "ki" => 10,
        "Mi" => 20,
        "Gi" => 30,
        "Ti" => 40,
        "Pi" => 50,
        "Ei" => 60,
        "Zi" => 70,
        "Yi" => 80,
        _ => return None,
    })
}

fn parse_unit(symbol: &str) -> Option<(Unit, f64)> {
    let symbol = if symbol.contains('^') { Cow::Owned(symbol.replace("^2", "2").replace("^3", "3")) } else { Cow::Borrowed(symbol) };
    if let Some(unit) = exact(&symbol) {
        return Some((unit, 1.0));
    }
    for size in [2, 1] {
        if let Some((prefix, bare)) = symbol.split_at_checked(size)
            && let Some(unit) = exact(bare)
            && unit.metric_power != 0
        {
            if unit.quantity == Quantity::Information
                && let Some(exponent) = binary_exponent(prefix)
            {
                return Some((unit, 2.0_f64.powi(exponent)));
            }
            if let Some(factor) = metric_factor(prefix) {
                let power = unit.metric_power;
                return Some((unit, factor.powi(power)));
            }
        }
    }
    None
}

pub(super) fn convert(args: &[Arg], _ctx: &mut dyn Ctx) -> R<Value> {
    let input = num(args, 0)?;
    let source_symbol = text(args, 1)?;
    let target_symbol = text(args, 2)?;
    let (source, source_prefix) = parse_unit(&source_symbol).ok_or(CellError::NA)?;
    let (target, target_prefix) = parse_unit(&target_symbol).ok_or(CellError::NA)?;

    if source.quantity != target.quantity {
        return Err(CellError::NA);
    }

    let converted = match (source.temperature, target.temperature) {
        (Some(from), Some(to)) if from == to && source_prefix == target_prefix => input,
        (Some(from), Some(to)) => to.reading(from.kelvin(input * source_prefix)) / target_prefix,
        _ => (input * source.scale * source_prefix) / (target.scale * target_prefix),
    };
    num_val(converted)
}
