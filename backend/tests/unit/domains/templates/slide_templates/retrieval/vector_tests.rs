use super::*;
#[test]
fn vectors_match_portable_float32_storage_and_reject_invalid_shapes() {
    assert_eq!(
        serialize_embedding(&[1.0, -2.0]).unwrap(),
        [0, 0, 128, 63, 0, 0, 0, 192]
    );
    assert_eq!(
        deserialize_embedding(&[0, 0, 128, 63, 0, 0, 0, 192], 2).unwrap(),
        [1.0, -2.0]
    );
    for vector in [&[][..], &[0.0, 0.0], &[f64::NAN], &[f64::INFINITY]] {
        assert!(validate_embedding(vector, None).is_err());
    }
    assert!(deserialize_embedding(&[0, 0, 0, 0], 2).is_err());
    assert!(cosine_similarity(&[1.0], &[1.0, 2.0]).is_err());
    assert!(
        (cosine_similarity(&[1.0, 1.0], &[1.0, 0.0]).unwrap() - 1.0 / 2.0_f64.sqrt()).abs() < 1e-10
    );
}
