use super::*;
use crate::domains::templates::slide_templates::retrieval::{
    PreparedSlideEmbeddings, TemplateEmbeddingProfile,
};

#[test]
fn provider_vectors_reject_invalid_shapes_before_storage() {
    for vector in [&[][..], &[0.0, 0.0], &[f64::NAN], &[f64::INFINITY]] {
        assert!(validate_embedding(vector, None).is_err());
    }
    assert!(validate_embedding(&[1.0, 0.0], Some(3)).is_err());
    assert!(validate_embedding(&[1.0, -2.0], Some(2)).is_ok());
}

#[test]
fn prepared_pairs_validate_both_roles_against_one_profile() {
    let profile = TemplateEmbeddingProfile::new("embedder", 2);
    let prepared = PreparedSlideEmbeddings::new(
        profile.clone(),
        "subject".into(),
        &[1.0, -2.0],
        "capability".into(),
        &[0.0, 1.0],
    )
    .unwrap();
    assert_eq!(prepared.subject.as_slice(), [1.0, -2.0]);
    assert_eq!(prepared.capability.as_slice(), [0.0, 1.0]);
    // One invalid role prevents the whole pair, including f32 underflow to zero.
    for (subject, capability) in [
        (&[1.0, 0.0][..], &[0.0, 0.0][..]),
        (&[1.0, 0.0], &[1.0, 0.0, 0.0]),
        (&[1e-50, 0.0], &[1.0, 0.0]),
        (&[f64::MAX, 0.0], &[1.0, 0.0]),
    ] {
        assert!(PreparedSlideEmbeddings::new(
            profile.clone(),
            "s".into(),
            subject,
            "c".into(),
            capability
        )
        .is_err());
    }
    assert!(PreparedSlideEmbeddings::new(
        profile,
        String::new(),
        &[1.0, 0.0],
        "c".into(),
        &[0.0, 1.0]
    )
    .is_err());
}

#[test]
fn profile_identity_covers_model_and_dimensions() {
    let base = TemplateEmbeddingProfile::new("embedder", 2);
    assert_eq!(base.id(), TemplateEmbeddingProfile::new("embedder", 2).id());
    assert_eq!(base.id().len(), 64);
    assert_ne!(base.id(), TemplateEmbeddingProfile::new("embedder", 3).id());
    assert_ne!(base.id(), TemplateEmbeddingProfile::new("other", 2).id());
}
