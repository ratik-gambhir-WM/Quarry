use super::*;
#[test]
fn number_strings_and_fixed_rounding_match_javascript() {
    let oracle: Value = serde_json::from_str(include_str!(
        "../../../../../fixtures/slide_templates/reference-fixture.json"
    ))
    .unwrap();
    for pair in oracle["rounded"].as_array().unwrap() {
        assert_eq!(
            round(pair[0].as_f64().unwrap()).parse::<f64>().unwrap(),
            pair[1].as_f64().unwrap()
        );
    }
    assert_eq!(
        stable_stringify(&serde_json::json!([1e-7, 1e-6, 1e20, 1e21, -0.0])),
        "[1e-7,0.000001,100000000000000000000,1e+21,0]"
    );
}
