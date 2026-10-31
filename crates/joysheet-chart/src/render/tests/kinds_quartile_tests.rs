use super::*;

#[test]
fn box_whisker_default_quartiles_are_exclusive() {
    let values = [1.0, 2.0, 3.0, 4.0, 5.0, 6.0];
    assert_eq!(quantile(&values, 0.25, QuartileMethod::default()), 1.75);
    assert_eq!(quantile(&values, 0.5, QuartileMethod::default()), 3.5);
    assert_eq!(quantile(&values, 0.75, QuartileMethod::default()), 5.25);
}

#[test]
fn quartile_methods_and_short_samples() {
    for (values, exclusive, inclusive) in [
        (vec![1., 2., 3.], (1., 3.), (1.5, 2.5)),
        (vec![1., 2., 3., 4.], (1.25, 3.75), (1.75, 3.25)),
        (vec![1., 2., 3., 4., 5.], (1.5, 4.5), (2., 4.)),
        (vec![1., 2., 3., 4., 5., 6., 7.], (2., 6.), (2.5, 5.5)),
        (vec![1., 1., 2., 3., 5., 8.], (1., 5.75), (1.25, 4.5)),
        (vec![1., 2.], (1., 2.), (1.25, 1.75)),
        (vec![1.], (1., 1.), (1., 1.)),
        (vec![], (0., 0.), (0., 0.)),
    ] {
        for (method, expected) in [(QuartileMethod::Exclusive, exclusive), (QuartileMethod::Inclusive, inclusive)] {
            assert_eq!((quantile(&values, 0.25, method), quantile(&values, 0.75, method)), expected);
        }
    }
}
