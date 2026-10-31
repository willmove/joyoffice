use super::*;
#[test]
fn sort_retains_region_and_rejects_bad_order() {
    let p = parameters("data.sort", json!({"column":"C","order":"desc","range":"A1:C8"})).unwrap();
    assert_eq!(p["keys"][0]["column"], "C");
    assert_eq!(p["range"], "A1:C8");
    assert!(parameters("data.sort", json!({"order":"bad"})).is_err());
}
#[test]
fn multiple_sort_keys_keep_priority_and_reject_invalid_directions() {
    let p = parameters("data.sort", json!({"column":"A","order":"asc","keysText":"B:desc, C:asc","range":"A1:C8","header":true})).unwrap();
    assert_eq!(p["keys"], json!([{"column":"A","order":"asc"},{"column":"B","order":"desc"},{"column":"C","order":"asc"}]));
    assert_eq!(p["range"], "A1:C8");
    assert!(parameters("data.sort", json!({"keysText":"B:sideways"})).is_err());
    assert!(p.get("keysText").is_none());
}
#[test]
fn columns_trim_and_keep_engine_arrays() {
    assert_eq!(parameters("data.removeDuplicates", json!({"columns":" A, C "})).unwrap()["columns"], json!(["A", "C"]));
    assert_eq!(parameters("data.removeDuplicates", json!({"columns":["B"]})).unwrap()["columns"], json!(["B"]));
}
