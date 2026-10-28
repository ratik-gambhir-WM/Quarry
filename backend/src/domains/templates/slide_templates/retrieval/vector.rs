/// Reject vectors that cannot safely participate in cosine similarity.
pub fn validate_embedding(vector: &[f64], dimensions: Option<usize>) -> anyhow::Result<()> {
    anyhow::ensure!(
        !vector.is_empty() && dimensions.is_none_or(|n| n == vector.len()),
        "Embedding dimensions do not match the configured dimensions."
    );
    let mut norm = 0.0;
    for &value in vector {
        anyhow::ensure!(value.is_finite(), "Embedding contains a non-finite value.");
        norm += value * value;
    }
    anyhow::ensure!(
        norm.is_finite() && norm != 0.0,
        "Embedding must have a finite non-zero norm."
    );
    Ok(())
}

pub fn serialize_embedding(vector: &[f64]) -> anyhow::Result<Vec<u8>> {
    validate_embedding(vector, None)?;
    Ok(vector
        .iter()
        .flat_map(|&value| (value as f32).to_le_bytes())
        .collect())
}

pub fn deserialize_embedding(bytes: &[u8], dimensions: usize) -> anyhow::Result<Vec<f64>> {
    anyhow::ensure!(
        dimensions > 0 && dimensions.checked_mul(4) == Some(bytes.len()),
        "Stored embedding dimensions are invalid."
    );
    let vector: Vec<_> = bytes
        .as_chunks::<4>()
        .0
        .iter()
        .map(|&bytes| f64::from(f32::from_le_bytes(bytes)))
        .collect();
    validate_embedding(&vector, Some(dimensions))?;
    Ok(vector)
}

pub fn cosine_similarity(left: &[f64], right: &[f64]) -> anyhow::Result<f64> {
    validate_embedding(left, None)?;
    validate_embedding(right, Some(left.len()))?;
    let (mut dot, mut ln, mut rn) = (0.0, 0.0, 0.0);
    for (&l, &r) in left.iter().zip(right) {
        dot += l * r;
        ln += l * l;
        rn += r * r;
    }
    Ok(dot / (ln * rn).sqrt())
}

#[cfg(test)]
#[path = "../../../../../tests/unit/domains/templates/slide_templates/retrieval/vector_tests.rs"]
mod tests;
