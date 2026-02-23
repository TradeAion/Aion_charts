//! TextWidthCache — bounded FIFO cache for Canvas2D text measurements.
//!
//! Mirrors LWC's TextWidthCache: digit normalization + bounded eviction.
//!
//! Key insight: in typical chart fonts, all digits 0-9 have equal width (tabular
//! figures). So "42,512.50" and "31,245.67" share the same pixel width. By
//! normalizing digits [1-9] -> '0' in the cache key, a single measurement can
//! serve all numeric strings of the same format pattern.
//!
//! The cache key also includes the font string to avoid cross-font collisions
//! (e.g. "11px monospace" vs "bold 11px sans-serif").
//!
//! Also caches `yMidCorrection` — the precise vertical centering offset computed
//! from `actualBoundingBoxAscent` / `actualBoundingBoxDescent`.
//! Matches LWC behavior by measuring with `textBaseline = "middle"`.

#![cfg(target_arch = "wasm32")]

use web_sys::CanvasRenderingContext2d;

/// Cached text measurement result.
#[derive(Debug, Clone, Copy)]
pub struct TextMeasurement {
    /// Text width in pixels.
    pub width: f64,
    /// yMidCorrection: vertical offset to apply when centering text.
    ///
    /// Computed as `(actualBoundingBoxAscent - actualBoundingBoxDescent) / 2`.
    /// When drawing with `textBaseline = "middle"`, add this to the Y coordinate
    /// for consistent vertical centering across browsers.
    pub y_mid_correction: f64,
}

/// Bounded FIFO cache for Canvas2D `measureText()` results.
///
/// - max_size: 50 (default) — enough for price labels, time labels, legend text.
/// - Eviction: oldest entry removed when cache is full.
/// - Digit normalization: [1-9] -> '0' in cache keys.
pub struct TextWidthCache {
    entries: Vec<(String, TextMeasurement)>,
    max_size: usize,
}

impl TextWidthCache {
    /// Create a new cache with the given max capacity.
    pub fn new(max_size: usize) -> Self {
        Self {
            entries: Vec::with_capacity(max_size),
            max_size,
        }
    }

    /// Measure text width via the cache. Returns pixel width.
    ///
    /// - On cache hit: returns the cached width immediately (no browser call).
    /// - On cache miss: calls `ctx.measureText(text).width`, caches the result
    ///   under the normalized key, evicts the oldest entry if full.
    ///
    /// `font_key` should be the current `ctx.font` string (e.g. "11px sans-serif").
    /// Pass it explicitly rather than reading from the context for performance.
    pub fn measure(&mut self, ctx: &CanvasRenderingContext2d, text: &str, font_key: &str) -> f64 {
        self.measure_full(ctx, text, font_key).width
    }

    /// Measure text and return the full measurement (width + yMidCorrection).
    ///
    /// Use `yMidCorrection` with `textBaseline = "middle"` for precise
    /// vertical centering of text in labels.
    pub fn measure_full(
        &mut self,
        ctx: &CanvasRenderingContext2d,
        text: &str,
        font_key: &str,
    ) -> TextMeasurement {
        let key = Self::make_key(font_key, text);

        // Linear scan is fine for max_size <= 50
        for &(ref k, m) in &self.entries {
            if *k == key {
                return m;
            }
        }

        // Cache miss -- measure via browser (LWC-compatible baseline handling)
        let measurement = {
            ctx.save();
            ctx.set_text_baseline("middle");
            let measured = ctx.measure_text(text);
            ctx.restore();

            match measured {
                Ok(m) => {
                    let ascent = m.actual_bounding_box_ascent();
                    let descent = m.actual_bounding_box_descent();
                    TextMeasurement {
                        width: m.width(),
                        y_mid_correction: (ascent - descent) / 2.0,
                    }
                }
                Err(_) => TextMeasurement {
                    width: 0.0,
                    y_mid_correction: 0.0,
                },
            }
        };

        // FIFO eviction
        if self.entries.len() >= self.max_size {
            self.entries.remove(0);
        }
        self.entries.push((key, measurement));

        measurement
    }

    /// Clear the cache (e.g. on DPR change or font change).
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// Current number of cached entries.
    #[allow(dead_code)]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Build the cache key: font + normalized text.
    ///
    /// Digit normalization: replace ASCII digits 1-9 with '0'.
    /// This exploits tabular-figure fonts where all digits are equal width.
    fn make_key(font: &str, text: &str) -> String {
        let mut key = String::with_capacity(font.len() + 1 + text.len());
        key.push_str(font);
        key.push('\0'); // separator

        for ch in text.chars() {
            if ch >= '1' && ch <= '9' {
                key.push('0');
            } else {
                key.push(ch);
            }
        }

        key
    }
}
