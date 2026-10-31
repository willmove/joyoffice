use super::*;

fn close(a: f64, b: f64, tol: f64) {
    assert!((a - b).abs() <= tol * b.abs().max(1.0), "{a} vs {b}");
}

#[test]
fn gamma_fn() {
    close(gamma(5.0), 24.0, 1e-15);
    close(gamma(0.5), PI.sqrt(), 1e-13);
    close(gamma(-1.5), 2.363_271_801_207_355, 1e-12);
    close(ln_gamma(10.0), 12.801_827_480_081_469, 1e-13);
    close(ln_gamma(101.0), 363.739_375_555_563_47, 1e-13);
    assert!(gamma(0.0).is_nan());
}

#[test]
fn error_fn() {
    close(erf(1.0), 0.842_700_792_949_714_9, 1e-14);
    close(erf(0.3), 0.328_626_759_459_127_4, 1e-14);
    close(erf(-2.0), -0.995_322_265_018_952_7, 1e-14);
    close(erfc(3.0), 2.209_049_699_858_544e-5, 1e-12);
    close(erfc(-1.0), 1.842_700_792_949_715, 1e-14);
}

#[test]
fn incomplete() {
    close(reg_gamma_p(2.0, 3.0), 0.800_851_726_528_544, 1e-13);
    close(reg_inc_beta(0.4, 2.0, 3.0), 0.5248, 1e-13);
    close(norm_cdf(1.96), 0.975_002_104_851_780, 1e-13);
    close(norm_inv(0.975), 1.959_963_984_540_054, 1e-12);
    close(norm_inv(1e-10), -6.361_340_902_404_056, 1e-10);
    close(invert(|x| x * x, 2.0, 0.0, 1.0, 10.0), 2f64.sqrt(), 1e-14);
}
