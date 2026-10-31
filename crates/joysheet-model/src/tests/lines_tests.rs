use super::*;
#[test]
fn shared_indexes_detach_on_all_mutations_and_roundtrip() {
    let mut a = Lines::default();
    a.insert(1, LineInfo { size: Some(40.), ..Default::default() });
    let first = a.index(20., 100);
    let mut b = a.clone();
    assert!(Arc::ptr_eq(&first, &b.index(20., 100)));
    b.entry(1).or_default().hidden = true;
    assert_eq!(b.index(20., 100).start(2), 20.);
    assert_eq!(a.index(20., 100).start(2), 60.);
    assert_eq!(a.index(30., 100).start(2), 70.);
    b.get_mut(&1).unwrap().hidden = false;
    assert_eq!(b.index(20., 100).start(2), 60.);
    b.retain(|_, _| false);
    assert_eq!(b.index(20., 100).start(2), 40.);
    let text = serde_json::to_string(&a).unwrap();
    assert_eq!(text, serde_json::to_string(&*a).unwrap());
    let loaded: Lines = serde_json::from_str(&text).unwrap();
    assert_eq!(loaded, a);
    assert_eq!(loaded.index(20., 100).start(2), 60.);
}
