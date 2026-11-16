use super::*;

#[test]
fn validates_storage_dimensions_independently_from_ann_limits() {
    for dimensions in [1, 1536, 3072, MAX_VECTOR_DIMENSIONS] {
        assert!(CosineVector::new(vec![1.0; dimensions], dimensions).is_ok());
    }
    for dimensions in [0, MAX_VECTOR_DIMENSIONS + 1] {
        assert_eq!(
            CosineVector::new(vec![1.0; dimensions], dimensions),
            Err(VectorError::Dimensions)
        );
    }
    assert_eq!(
        CosineVector::new(vec![1.0], 1536),
        Err(VectorError::ProfileDimensions)
    );
}

#[test]
fn rejects_invalid_values_before_and_after_narrowing() {
    for values in [
        vec![0.0, -0.0],
        vec![f64::NAN, 1.0],
        vec![f64::INFINITY, 1.0],
        vec![f64::NEG_INFINITY, 1.0],
        vec![f64::MAX, 1.0],
        vec![f64::from(f32::MAX) * 2.0, 1.0],
        vec![1e-50, -1e-50],
    ] {
        assert_eq!(
            CosineVector::from_f64(&values, 2),
            Err(VectorError::InvalidValues)
        );
    }
    for values in [
        vec![0.0],
        vec![f32::NAN],
        vec![f32::INFINITY],
        vec![f32::MAX],
        vec![f32::MIN_POSITIVE],
    ] {
        assert_eq!(
            CosineVector::new(values, 1),
            Err(VectorError::InvalidValues)
        );
    }
    let vector = CosineVector::from_f64(&[0.25, -0.5, 1.0], 3).unwrap();
    assert_eq!(vector.as_slice(), &[0.25, -0.5, 1.0]);
    assert!(CosineVector::new(vec![1e-10], 1).is_ok());
}

#[test]
fn debug_output_exposes_dimensions_without_values() {
    let vector = CosineVector::new(vec![123.5, -987.5], 2).unwrap();
    let debug = format!("{vector:?}");
    assert!(debug.contains("dimensions: 2"));
    assert!(!debug.contains("123.5"));
    assert!(!format!("{:?}", super::super::query::SqlValue::from(vector)).contains("987.5"));
}
