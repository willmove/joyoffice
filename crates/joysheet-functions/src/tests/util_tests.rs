use super::*;

#[test]
fn rounding() {
    assert_eq!(round_half_away(2.675, 2), 2.68);
    assert_eq!(round_half_away(-2.5, 0), -3.0);
    assert_eq!(round_half_away(1234.5678, -2), 1200.0);
    assert_eq!(round_half_away(0.285, 2), 0.29);
    assert_eq!(round_half_away(1.005, 2), 1.01);
    assert_eq!(round_half_away(0.1 + 0.2, 15), 0.3);
    assert_eq!(round_half_away(0.1 + 0.2, 20), 0.3);
}

#[test]
fn aggregate_numbers() {
    let arr = Value::from(Array::new(1, 4, vec![Value::Number(1.0), Value::from("2"), Value::Bool(true), Value::Empty]).unwrap());
    assert_eq!(numbers(&[Arg::reference(arr.clone())]).unwrap(), vec![1.0]);
    assert_eq!(numbers_a(&[Arg::reference(arr)]).unwrap(), vec![1.0, 0.0, 1.0]);
    assert_eq!(numbers(&[Arg::val("3"), Arg::val(true)]).unwrap(), vec![3.0, 1.0]);
    assert_eq!(numbers(&[Arg::val("x")]), Err(CellError::Value));
    assert_eq!(numbers(&[Arg::reference("x")]).unwrap(), Vec::<f64>::new());
}
