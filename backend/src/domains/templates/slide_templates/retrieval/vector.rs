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

#[cfg(test)]
#[path = "../../../../../tests/unit/domains/templates/slide_templates/retrieval/vector_tests.rs"]
mod tests;
