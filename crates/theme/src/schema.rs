#![allow(missing_docs)]

use anyhow::{Context as _, bail};
use gpui::{Hsla, Rgba};
use palette::FromColor;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::color_space::{Oklch, oklch_to_rgba};

/// The appearance of a theme in serialized content.
#[derive(Debug, PartialEq, Clone, Copy, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum AppearanceContent {
    Light,
    Dark,
}

/// Parses a color string into an [`Hsla`] value.
///
/// Accepts hex colors as well as `oklch(l c h)` and `oklch(l c h / a)` with
/// plain decimal components.
pub fn try_parse_color(color: &str) -> anyhow::Result<Hsla> {
    if let Some(hsla) = try_parse_oklch(color)? {
        return Ok(hsla);
    }

    let rgba = Rgba::try_from(color)?;
    Ok(rgba_to_hsla(rgba))
}

fn rgba_to_hsla(rgba: Rgba) -> Hsla {
    let rgba = palette::rgb::Srgba::from_components((rgba.r, rgba.g, rgba.b, rgba.a));
    let hsla = palette::Hsla::from_color(rgba);

    gpui::hsla(
        hsla.hue.into_positive_degrees() / 360.,
        hsla.saturation,
        hsla.lightness,
        hsla.alpha,
    )
}

const OKLCH_FORMAT: &str =
    "Expected oklch(l c h) or oklch(l c h / a) with plain decimal components";

fn try_parse_oklch(color: &str) -> anyhow::Result<Option<Hsla>> {
    let Some(oklch) = color.trim().strip_prefix("oklch(") else {
        return Ok(None);
    };
    let oklch = oklch
        .strip_suffix(')')
        .with_context(|| format!("invalid oklch color: '{color}'. {OKLCH_FORMAT}"))?;

    let (channels, alpha) = match oklch.split_once('/') {
        Some((channels, alpha)) => (channels, alpha.trim()),
        None => (oklch, "1"),
    };
    let channels = channels.split_whitespace().collect::<Vec<_>>();
    let [lightness, chroma, hue] = channels.as_slice() else {
        bail!("invalid oklch color: '{color}'. {OKLCH_FORMAT}");
    };

    let parse = |component: &str, name: &str| -> anyhow::Result<f32> {
        component
            .parse::<f32>()
            .with_context(|| format!("invalid {name} in oklch color: '{color}'"))
    };
    let lightness = parse(lightness, "lightness")?;
    let chroma = parse(chroma, "chroma")?;
    let hue = parse(hue, "hue")?;
    let alpha = parse(alpha, "alpha")?;

    if ![lightness, chroma, hue, alpha]
        .into_iter()
        .all(f32::is_finite)
    {
        bail!("oklch components must be finite numbers: '{color}'");
    }

    let rgba = oklch_to_rgba(
        Oklch {
            l: lightness.clamp(0.0, 1.0),
            chroma: chroma.max(0.0),
            hue: hue.rem_euclid(360.0),
        },
        alpha.clamp(0.0, 1.0),
    );

    Ok(Some(rgba_to_hsla(Rgba {
        r: quantize(rgba.r),
        g: quantize(rgba.g),
        b: quantize(rgba.b),
        a: quantize(rgba.a),
    })))
}

// Snap channels to the 8-bit grid so oklch colors match their hex equivalents
fn quantize(channel: f32) -> f32 {
    (channel * 255.0).round() / 255.0
}

#[cfg(test)]
mod tests {
    use gpui::{Hsla, rgba};

    use super::try_parse_color;

    #[test]
    fn parses_oklch_to_quantized_srgb() {
        assert_eq!(
            try_parse_color("oklch(0.7 0.15 200)").unwrap(),
            Hsla::from(rgba(0x00b9c3ff))
        );
        assert_eq!(
            try_parse_color("oklch(0.7 0.15 200 / 0.5)").unwrap(),
            Hsla::from(rgba(0x00b9c380))
        );
    }

    #[test]
    fn rejects_invalid_oklch() {
        for color in [
            "oklch(0.7 0.15)",
            "oklch(0.7 0.15 200",
            "oklch(70% 0.15 200)",
            "oklch(0.7 0.15 200 / 50%)",
            "oklch(0.7 0.15 200 0.5)",
            "oklch(0.7 0.15 NaN)",
        ] {
            assert!(try_parse_color(color).is_err(), "expected {color} to fail");
        }
    }
}
