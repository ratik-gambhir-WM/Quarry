use super::validate::{validate_expected_generation, validate_file_id};

#[test]
fn file_ids_must_be_content_addresses() {
    assert!(validate_file_id(&"a".repeat(64)).is_ok());
    for invalid in [
        "",
        "file-1",
        "5346b615-818b-4b25-85da-045507df2db4",
        &"A".repeat(64),
        &"a".repeat(65),
    ] {
        assert!(validate_file_id(invalid).is_err(), "{invalid}");
    }
}

#[test]
fn expected_generation_is_a_non_negative_safe_integer() {
    assert!(validate_expected_generation(0).is_ok());
    assert!(validate_expected_generation(7).is_ok());
    assert!(validate_expected_generation(-1).is_err());
    assert!(validate_expected_generation(9_007_199_254_740_991).is_err());
}
