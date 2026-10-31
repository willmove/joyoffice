use joysheet_core::{CellError, Value};

use super::parse_complex;
use crate::util::testutil::*;

#[track_caller]
fn cx(v: Value, re: f64, im: f64) {
    let Value::Text(s) = &v else { panic!("expected complex text, got {v:?}") };
    let (z, _) = parse_complex(s).unwrap();
    let tol = 1e-12;
    assert!((z.re - re).abs() <= tol * re.abs().max(1.0), "{s}: re {} vs {re}", z.re);
    assert!((z.im - im).abs() <= tol * im.abs().max(1.0), "{s}: im {} vs {im}", z.im);
}

#[test]
fn base_conversions() {
    close(ev("BIN2DEC", vec![n(1100100.0)]), 100.0);
    close(ev("BIN2DEC", vec![n(1111111111.0)]), -1.0);
    is_text(ev("BIN2HEX", vec![n(11111011.0), n(4.0)]), "00FB");
    is_text(ev("BIN2HEX", vec![n(1110.0)]), "E");
    is_text(ev("BIN2HEX", vec![n(1111111111.0)]), "FFFFFFFFFF");
    is_text(ev("BIN2OCT", vec![n(1001.0), n(3.0)]), "011");
    is_text(ev("BIN2OCT", vec![n(1111111111.0)]), "7777777777");
    is_text(ev("DEC2BIN", vec![n(9.0), n(4.0)]), "1001");
    is_text(ev("DEC2BIN", vec![n(-100.0)]), "1110011100");
    is_err(ev("DEC2BIN", vec![n(512.0)]), CellError::Num);
    is_text(ev("DEC2HEX", vec![n(100.0), n(4.0)]), "0064");
    is_text(ev("DEC2HEX", vec![n(-54.0)]), "FFFFFFFFCA");
    is_text(ev("DEC2HEX", vec![n(28.0)]), "1C");
    is_err(ev("DEC2HEX", vec![n(64.0), n(1.0)]), CellError::Num);
    is_text(ev("DEC2OCT", vec![n(58.0), n(3.0)]), "072");
    is_text(ev("DEC2OCT", vec![n(-100.0)]), "7777777634");
    is_text(ev("HEX2BIN", vec![t("F"), n(8.0)]), "00001111");
    is_text(ev("HEX2BIN", vec![t("B7")]), "10110111");
    is_text(ev("HEX2BIN", vec![t("FFFFFFFE00")]), "1000000000");
    is_err(ev("HEX2BIN", vec![t("200")]), CellError::Num);
    close(ev("HEX2DEC", vec![t("A5")]), 165.0);
    close(ev("HEX2DEC", vec![t("FFFFFFFF5B")]), -165.0);
    close(ev("HEX2DEC", vec![t("3DA408B9")]), 1034160313.0);
    is_text(ev("HEX2OCT", vec![t("F"), n(3.0)]), "017");
    is_text(ev("HEX2OCT", vec![t("3B4E")]), "35516");
    is_text(ev("HEX2OCT", vec![t("FFFFFFFF00")]), "7777777400");
    is_text(ev("OCT2BIN", vec![n(3.0), n(3.0)]), "011");
    is_text(ev("OCT2BIN", vec![n(7777777000.0)]), "1000000000");
    close(ev("OCT2DEC", vec![n(54.0)]), 44.0);
    close(ev("OCT2DEC", vec![n(7777777533.0)]), -165.0);
    is_text(ev("OCT2HEX", vec![n(100.0), n(4.0)]), "0040");
    is_text(ev("OCT2HEX", vec![n(7777777533.0)]), "FFFFFFFF5B");
    is_err(ev("BIN2DEC", vec![n(102.0)]), CellError::Num);
    is_err(ev("HEX2DEC", vec![t("GG")]), CellError::Num);
    is_err(ev("BIN2DEC", vec![t("11111111111")]), CellError::Num);
    is_err(ev("DEC2BIN", vec![t("abc")]), CellError::Value);
    is_text(ev("DEC2BIN", vec![n(9.7)]), "1001");
    close(ev("HEX2DEC", vec![t("ff")]), 255.0);
}

#[test]
fn bits() {
    close(ev("BITAND", vec![n(1.0), n(5.0)]), 1.0);
    close(ev("BITAND", vec![n(13.0), n(25.0)]), 9.0);
    close(ev("BITOR", vec![n(23.0), n(10.0)]), 31.0);
    close(ev("BITXOR", vec![n(5.0), n(3.0)]), 6.0);
    close(ev("BITLSHIFT", vec![n(4.0), n(2.0)]), 16.0);
    close(ev("BITRSHIFT", vec![n(13.0), n(2.0)]), 3.0);
    close(ev("BITRSHIFT", vec![n(13.0), n(-2.0)]), 52.0);
    is_err(ev("BITAND", vec![n(-1.0), n(5.0)]), CellError::Num);
    is_err(ev("BITAND", vec![n(1.5), n(5.0)]), CellError::Num);
    is_err(ev("BITLSHIFT", vec![n(1.0), n(54.0)]), CellError::Num);
    is_err(ev("BITLSHIFT", vec![n(2f64.powi(47)), n(1.0)]), CellError::Num);
}

#[test]
fn convert_units() {
    close(ev("CONVERT", vec![n(1.0), t("lbm"), t("kg")]), 0.45359237);
    close(ev("CONVERT", vec![n(68.0), t("F"), t("C")]), 20.0);
    close(ev("CONVERT", vec![n(100.0), t("C"), t("F")]), 212.0);
    close(ev("CONVERT", vec![n(0.0), t("C"), t("K")]), 273.15);
    close(ev("CONVERT", vec![n(1.0), t("in"), t("cm")]), 2.54);
    close(ev("CONVERT", vec![n(1.0), t("mi"), t("km")]), 1.609344);
    close(ev("CONVERT", vec![n(1.0), t("gal"), t("l")]), 3.785411784);
    close(ev("CONVERT", vec![n(1.0), t("byte"), t("bit")]), 8.0);
    close(ev("CONVERT", vec![n(1.0), t("kibyte"), t("byte")]), 1024.0);
    close(ev("CONVERT", vec![n(100.0), t("km/h"), t("m/s")]), 100.0 / 3.6);
    close(ev("CONVERT", vec![n(1.0), t("m2"), t("ft2")]), 10.763910416709722);
    close(ev("CONVERT", vec![n(1.0), t("km2"), t("m2")]), 1e6);
    close(ev("CONVERT", vec![n(1.0), t("hr"), t("mn")]), 60.0);
    close(ev("CONVERT", vec![n(1.0), t("atm"), t("Pa")]), 101325.0);
    close(ev("CONVERT", vec![n(1.0), t("ha"), t("m^2")]), 10000.0);
    close(ev("CONVERT", vec![n(1.0), t("cal"), t("J")]), 4.1868);
    close(ev("CONVERT", vec![n(1.0), t("mm"), t("m")]), 0.001);
    close(ev("CONVERT", vec![n(6.0), t("ft"), t("yd")]), 2.0);
    close(ev("CONVERT", vec![n(1.0), t("cup"), t("tsp")]), 48.0);
    is_err(ev("CONVERT", vec![n(2.5), t("ft"), t("sec")]), CellError::NA);
    is_err(ev("CONVERT", vec![n(1.0), t("xyz"), t("m")]), CellError::NA);
    is_err(ev("CONVERT", vec![n(1.0), t("kft"), t("m")]), CellError::NA);
    is_err(ev("CONVERT", vec![n(1.0), t("LBM"), t("kg")]), CellError::NA);
}

#[test]
fn delta_gestep_erf() {
    close(ev("DELTA", vec![n(5.0), n(4.0)]), 0.0);
    close(ev("DELTA", vec![n(5.0), n(5.0)]), 1.0);
    close(ev("DELTA", vec![n(0.5)]), 0.0);
    close(ev("GESTEP", vec![n(5.0), n(4.0)]), 1.0);
    close(ev("GESTEP", vec![n(-4.0)]), 0.0);
    close_tol(ev("ERF", vec![n(0.745)]), 0.707_928_920, 1e-9);
    close_tol(ev("ERF", vec![n(1.0)]), 0.842_700_792_949_715, 1e-12);
    close_tol(ev("ERF", vec![n(0.0), n(1.5)]), 0.966_105_146_475_311, 1e-12);
    close_tol(ev("ERF.PRECISE", vec![n(1.0)]), 0.842_700_792_949_715, 1e-12);
    close_tol(ev("ERFC", vec![n(1.0)]), 0.157_299_207_050_285, 1e-12);
    close_tol(ev("ERFC.PRECISE", vec![n(-1.0)]), 1.842_700_792_949_715, 1e-12);
}

#[test]
fn complex_numbers() {
    is_text(ev("COMPLEX", vec![n(3.0), n(4.0)]), "3+4i");
    is_text(ev("COMPLEX", vec![n(3.0), n(4.0), t("j")]), "3+4j");
    is_text(ev("COMPLEX", vec![n(0.0), n(1.0)]), "i");
    is_text(ev("COMPLEX", vec![n(0.0), n(-1.0)]), "-i");
    is_text(ev("COMPLEX", vec![n(1.5), n(-2.0)]), "1.5-2i");
    is_text(ev("COMPLEX", vec![n(3.0), n(0.0)]), "3");
    is_text(ev("COMPLEX", vec![n(0.0), n(0.0)]), "0");
    is_err(ev("COMPLEX", vec![n(1.0), n(1.0), t("k")]), CellError::Value);
    close(ev("IMABS", vec![t("5+12i")]), 13.0);
    close(ev("IMREAL", vec![t("6-9i")]), 6.0);
    close(ev("IMAGINARY", vec![t("3+4i")]), 4.0);
    close(ev("IMAGINARY", vec![t("-j")]), -1.0);
    close(ev("IMAGINARY", vec![t("4")]), 0.0);
    close(ev("IMREAL", vec![n(7.0)]), 7.0);
    close(ev("IMARGUMENT", vec![t("3+4i")]), 0.927_295_218_001_612_2);
    is_err(ev("IMARGUMENT", vec![t("0")]), CellError::Div0);
    is_text(ev("IMCONJUGATE", vec![t("3+4i")]), "3-4i");
    is_text(ev("IMDIV", vec![t("-238+240i"), t("10+24i")]), "5+12i");
    is_err(ev("IMDIV", vec![t("1+i"), t("0")]), CellError::Num);
    is_text(ev("IMPRODUCT", vec![t("3+4i"), t("5-3i")]), "27+11i");
    is_text(ev("IMPRODUCT", vec![t("1+2i"), n(30.0)]), "30+60i");
    is_text(ev("IMSUM", vec![t("3+4i"), t("5-3i")]), "8+i");
    is_text(ev("IMSUM", vec![rf(arr(vec![vec![tv("1+i"), Value::Empty, tv("2")]]))]), "3+i");
    is_text(ev("IMSUB", vec![t("13+4i"), t("5+3i")]), "8+i");
    is_err(ev("IMSUM", vec![t("1+i"), t("1+j")]), CellError::Value);
    is_err(ev("IMABS", vec![t("abc")]), CellError::Num);
    is_err(ev("IMABS", vec![b(true)]), CellError::Value);
    cx(ev("IMEXP", vec![t("1+i")]), 1.468_693_939_915_885_2, 2.287_355_287_178_842);
    cx(ev("IMLN", vec![t("3+4i")]), 1.609_437_912_434_100_3, 0.927_295_218_001_612_2);
    cx(ev("IMLOG10", vec![t("3+4i")]), 0.698_970_004_336_018_8, 0.402_719_196_273_373_2);
    cx(ev("IMLOG2", vec![t("3+4i")]), 2.321_928_094_887_362, 1.337_804_212_450_976_8);
    cx(ev("IMSQRT", vec![t("1+i")]), 1.098_684_113_467_809_9, 0.455_089_860_562_227_3);
    cx(ev("IMSQRT", vec![t("-4")]), 0.0, 2.0);
    is_text(ev("IMSQRT", vec![t("-4")]), "2i");
    is_text(ev("IMSQRT", vec![t("-9")]), "3i");
    is_text(ev("IMSQRT", vec![t("4")]), "2");
    is_text(ev("IMSQRT", vec![t("-2i")]), "1-i");
    is_text(ev("IMPOWER", vec![t("2+3i"), n(3.0)]), "-46+9i");
    cx(ev("IMPOWER", vec![t("2+3i"), n(0.5)]), 1.674_149_228_035_540_2, 0.895_977_476_129_838);
    cx(ev("IMSIN", vec![t("3+4i")]), 3.853_738_037_919_377, -27.016_813_258_003_93);
    cx(ev("IMCOS", vec![t("1+i")]), 0.833_730_025_131_149, -0.988_897_705_762_865);
    cx(ev("IMSINH", vec![t("4+3i")]), -27.016_813_258_003_93, 3.853_738_037_919_377);
    cx(ev("IMCOSH", vec![t("4+3i")]), -27.034_945_603_074_224, 3.851_153_334_811_777);
    cx(ev("IMTAN", vec![t("4+3i")]), 0.004_908_258_067_496_06, 1.000_709_536_067_233);
    cx(ev("IMCOT", vec![t("4+3i")]), 0.004_901_182_394_304_4, -0.999_266_927_805_902);
    cx(ev("IMSEC", vec![t("4+3i")]), -0.065_294_027_857_947_1, -0.075_224_960_302_773_2);
    cx(ev("IMCSC", vec![t("4+3i")]), -0.075_489_832_915_863_7, 0.064_877_471_370_635_5);
    is_text(ev("IMSECH", vec![t("0")]), "1");
    is_err(ev("IMCSCH", vec![t("0")]), CellError::Num);
    // sech z · cosh z = 1
    let Value::Text(c) = ev("IMCOSH", vec![t("4+3i")]) else { panic!() };
    let Value::Text(sc) = ev("IMSECH", vec![t("4+3i")]) else { panic!() };
    cx(ev("IMPRODUCT", vec![t(&c), t(&sc)]), 1.0, 0.0);
    let Value::Text(sh) = ev("IMSINH", vec![t("4+3i")]) else { panic!() };
    let Value::Text(csh) = ev("IMCSCH", vec![t("4+3i")]) else { panic!() };
    cx(ev("IMPRODUCT", vec![t(&sh), t(&csh)]), 1.0, 0.0);
    cx(ev("IMSUM", vec![t("1.5e1+2E-1j")]), 15.0, 0.2);
}

#[test]
fn bessel() {
    close_tol(ev("BESSELJ", vec![n(1.9), n(2.0)]), 0.329_925_829, 1e-6);
    close_tol(ev("BESSELY", vec![n(2.5), n(1.0)]), 0.145_918_138, 1e-6);
    close_tol(ev("BESSELI", vec![n(1.5), n(1.0)]), 0.981_666_428, 1e-6);
    close_tol(ev("BESSELK", vec![n(1.5), n(1.0)]), 0.277_387_804, 1e-6);
    close_tol(ev("BESSELJ", vec![n(1.0), n(0.0)]), 0.765_197_686_557_966_6, 1e-12);
    close_tol(ev("BESSELJ", vec![n(1.0), n(1.0)]), 0.440_050_585_744_933_5, 1e-12);
    close_tol(ev("BESSELJ", vec![n(-1.0), n(1.0)]), -0.440_050_585_744_933_5, 1e-12);
    close_tol(ev("BESSELY", vec![n(1.0), n(0.0)]), 0.088_256_964_215_676_96, 1e-11);
    close_tol(ev("BESSELY", vec![n(1.0), n(1.0)]), -0.781_212_821_300_288_7, 1e-11);
    close_tol(ev("BESSELI", vec![n(1.0), n(0.0)]), 1.266_065_877_752_008_4, 1e-12);
    close_tol(ev("BESSELI", vec![n(1.0), n(1.0)]), 0.565_159_103_992_485_1, 1e-12);
    close_tol(ev("BESSELK", vec![n(1.0), n(0.0)]), 0.421_024_438_240_708_3, 1e-11);
    close_tol(ev("BESSELK", vec![n(1.0), n(1.0)]), 0.601_907_230_197_234_6, 1e-11);
    is_err(ev("BESSELJ", vec![n(1.0), n(-1.0)]), CellError::Num);
    is_err(ev("BESSELY", vec![n(0.0), n(1.0)]), CellError::Num);
    is_err(ev("BESSELK", vec![n(-1.0), n(1.0)]), CellError::Num);
    is_err(ev("BESSELJ", vec![t("x"), n(1.0)]), CellError::Value);
    // Wronskian J_{1}Y_0 − J_0Y_1 = 2/(πx), checking series and asymptotic regimes.
    for x in [0.5, 7.0, 15.0, 24.0, 30.0, 80.0] {
        let j0 = match ev("BESSELJ", vec![n(x), n(0.0)]) {
            Value::Number(v) => v,
            _ => panic!(),
        };
        let j1 = match ev("BESSELJ", vec![n(x), n(1.0)]) {
            Value::Number(v) => v,
            _ => panic!(),
        };
        let y0 = match ev("BESSELY", vec![n(x), n(0.0)]) {
            Value::Number(v) => v,
            _ => panic!(),
        };
        let y1 = match ev("BESSELY", vec![n(x), n(1.0)]) {
            Value::Number(v) => v,
            _ => panic!(),
        };
        let w = j1 * y0 - j0 * y1;
        let expect = 2.0 / (std::f64::consts::PI * x);
        assert!((w - expect).abs() < 1e-9 * expect.max(1.0), "x={x}: {w} vs {expect}");
    }
    // Large orders and arguments stay finite.
    assert!(matches!(ev("BESSELJ", vec![n(50.0), n(3.0)]), Value::Number(_)));
    assert!(matches!(ev("BESSELK", vec![n(30.0), n(5.0)]), Value::Number(_)));
}
