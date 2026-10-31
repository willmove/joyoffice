use super::*;
#[test]
fn scopes_are_optional_isolated_and_reset_between_requests() {
    {
        let _s = scope(Phase::RangeRead);
    }
    assert_eq!(finish().ranges, 0);
    start();
    {
        let _s = scope(Phase::Recalc);
        let _r = scope(Phase::RangeRead);
        formulas(7);
    }
    let other = std::thread::spawn(|| {
        start();
        {
            let _r = scope(Phase::RangeRead);
        }
        finish()
    })
    .join()
    .unwrap();
    let own = finish();
    assert_eq!(own.formulas, 7);
    assert_eq!(own.ranges, 1);
    assert_eq!(other.formulas, 0);
    assert_eq!(other.ranges, 1);
    assert!(own.recalc_ms >= own.range_read_ms && own.range_read_ms >= 0.);
    assert_eq!(finish().formulas, 0);
}
