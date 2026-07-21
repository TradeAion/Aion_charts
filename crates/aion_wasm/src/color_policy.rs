//! Per-direction color-override resolution for candlestick wick/border colors.
//!
//! The wasm boundary distinguishes three JS inputs so a pinned color can be *un-pinned* back to
//! following the body color (LWC parity): `undefined` = keep the current value, `""` = clear the
//! override (follow the direction's body color), any parseable CSS color = pin it. An unparseable
//! color is ignored (keeps the current value).

use aion_render::color::Color;

/// Apply an `Option<String>` color input to an override `slot`. See the module docs for the
/// keep/clear/pin contract.
pub(crate) fn update_color_slot(slot: &mut Option<Color>, value: Option<String>) {
    match value {
        None => {}
        Some(v) if v.is_empty() => *slot = None,
        Some(v) => {
            if let Some(c) = Color::parse_css(&v) {
                *slot = Some(c);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_clears_and_pins() {
        let mut slot: Option<Color> = None;

        // `undefined` (None) leaves the slot untouched.
        update_color_slot(&mut slot, None);
        assert_eq!(slot, None);

        // A CSS color pins the override.
        update_color_slot(&mut slot, Some("#ff0000".to_string()));
        assert_eq!(slot, Some(Color::rgb(0xff, 0x00, 0x00)));

        // `undefined` still keeps the pinned color (the case the TS `?? ""` fallback broke).
        update_color_slot(&mut slot, None);
        assert_eq!(slot, Some(Color::rgb(0xff, 0x00, 0x00)));

        // An empty string clears the override back to "follow the body color".
        update_color_slot(&mut slot, Some(String::new()));
        assert_eq!(slot, None);

        // An unparseable color is ignored, keeping whatever was there.
        update_color_slot(&mut slot, Some("#00ff00".to_string()));
        update_color_slot(&mut slot, Some("not-a-color".to_string()));
        assert_eq!(slot, Some(Color::rgb(0x00, 0xff, 0x00)));
    }
}
