//! Shared validation helpers for series data arrays.

#[inline]
pub fn ensure_equal_len(
    name_a: &str,
    len_a: usize,
    name_b: &str,
    len_b: usize,
) -> Result<(), String> {
    if len_a != len_b {
        Err(format!(
            "{} and {} length mismatch: {} != {}",
            name_a, name_b, len_a, len_b
        ))
    } else {
        Ok(())
    }
}

#[inline]
pub fn ensure_strictly_increasing_timestamps(name: &str, timestamps: &[u64]) -> Result<(), String> {
    for i in 1..timestamps.len() {
        if timestamps[i] <= timestamps[i - 1] {
            return Err(format!(
                "{} timestamps must be strictly increasing at index {}: {} <= {}",
                name,
                i,
                timestamps[i],
                timestamps[i - 1]
            ));
        }
    }
    Ok(())
}
