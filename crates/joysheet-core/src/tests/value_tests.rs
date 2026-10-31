use super::*;

#[test]
fn number_text() {
    assert_eq!(number_to_text(1.0), "1");
    assert_eq!(number_to_text(-2.5), "-2.5");
    assert_eq!(number_to_text(0.1 + 0.2), "0.3");
    assert_eq!(number_to_text(123456789012345.0), "123456789012345");
    assert_eq!(number_to_text(1e15), "1E+15");
    assert_eq!(number_to_text(1234567890123456.0), "1.23456789012346E+15");
    assert_eq!(number_to_text(0.0001), "0.0001");
    assert_eq!(number_to_text(1e-10), "1E-10");
    assert_eq!(number_to_text(1.0 / 3.0), "0.333333333333333");
}

#[test]
fn number_text_rounds_up_to_scientific_boundary() {
    assert_eq!(number_to_text(999999999999999.9), "1E+15");
    assert_eq!(number_to_text(-999999999999999.9), "-1E+15");
    assert_eq!(number_to_text(999999999999999.0), "999999999999999");
}

#[test]
fn coercion() {
    assert_eq!(Value::from("12").to_number(), Ok(12.0));
    assert_eq!(Value::from(" 1,234.5 ").to_number(), Ok(1234.5));
    assert_eq!(Value::from("50%").to_number(), Ok(0.5));
    assert_eq!(Value::from("abc").to_number(), Err(CellError::Value));
    assert_eq!(Value::Bool(true).to_number(), Ok(1.0));
    assert_eq!(Value::Empty.to_text(), Ok(String::new()));
    assert_eq!(Value::from("true").to_bool(), Ok(true));
    assert!(Value::number(f64::NAN).is_error());
}

#[test]
fn comparisons() {
    assert_eq!(compare(&Value::from("abc"), &Value::from("ABC")), Ordering::Equal);
    assert_eq!(compare(&Value::Number(5.0), &Value::from("1")), Ordering::Less);
    assert_eq!(compare(&Value::from("z"), &Value::Bool(false)), Ordering::Less);
    assert_eq!(compare(&Value::Empty, &Value::Number(0.0)), Ordering::Equal);
    assert_eq!(sort_compare(&Value::Empty, &Value::Number(1.0)), Ordering::Greater);
}

#[test]
fn errors() {
    for e in CellError::ALL {
        assert_eq!(CellError::parse(e.as_str()), Some(e));
    }
    assert_eq!(CellError::parse("#div/0!"), Some(CellError::Div0));
}

#[test]
fn numbers_compare_at_15_significant_digits() {
    let num = Value::Number;
    assert_eq!(compare(&num(0.1 + 0.2), &num(0.3)), Ordering::Equal);
    assert_eq!(compare(&num(1.1 * 3.0), &num(3.3)), Ordering::Equal);
    assert_eq!(compare(&num(1.0), &num(1.0 + 1e-16)), Ordering::Equal);
    assert_ne!(compare(&num(0.1 + 0.2), &num(0.3)), Ordering::Greater);
    assert_eq!(compare(&num(1.0), &num(1.00000000000001)), Ordering::Less);
    assert_eq!(compare(&num(1.0), &num(2.0)), Ordering::Less);
}

#[test]
fn arrays() {
    let a = Array::new(2, 2, vec![1.0.into(), 2.0.into(), 3.0.into(), 4.0.into()]).unwrap();
    assert_eq!(a.transpose().get(0, 1), Some(&Value::Number(3.0)));
    assert!(Array::new(2, 2, vec![]).is_none());
    let row = Array::row(vec![1.0.into(), 2.0.into()]);
    assert_eq!(row.get_broadcast(5, 1), Value::Number(2.0));
}
