// Copyright 2024 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Implementation of the CSS font matching algorithm.
//!
//! See: <https://drafts.csswg.org/css-fonts/#font-style-matching>

use super::font::{DEFAULT_OBLIQUE_ANGLE, FontInfo};
use crate::{AttrRange, FontStyle, FontWeight, FontWidth};
use smallvec::SmallVec;

pub fn match_font(
    set: &[FontInfo],
    width: FontWidth,
    style: FontStyle,
    weight: FontWeight,
    synthesize_style: bool,
) -> Option<usize> {
    match set.len() {
        0 => return None,
        1 => return Some(0),
        _ => {}
    }
    let set: CandidateVec = set
        .iter()
        .enumerate()
        .map(|(i, font)| Candidate {
            index: i,
            width: AttrRange::new(
                font.width().min().percentage(),
                font.width().max().percentage(),
            ),
            style: CandidateStyle::from_range(font.style()),
            weight: AttrRange::new(font.weight().min().value(), font.weight().max().value()),
            has_slnt: font.has_slant_axis(),
        })
        .collect();
    match_candidates(
        set,
        width.percentage(),
        style,
        weight.value(),
        synthesize_style,
    )
}

type CandidateVec = SmallVec<[Candidate; 16]>;

/// A font in the matching set, with its attributes reduced to ranges of
/// primitive values.
///
/// Non-variable fonts support a trivial range (`min == max`) for each
/// attribute while variable fonts support the range of the corresponding
/// variation axis.
#[derive(Copy, Clone, Debug)]
struct Candidate {
    index: usize,
    /// Supported widths as percentages (`100.0` is normal).
    width: AttrRange<f32>,
    style: CandidateStyle,
    weight: AttrRange<f32>,
    has_slnt: bool,
}

/// The set of styles supported by a font in the matching set.
#[derive(Copy, Clone, PartialEq, Debug)]
enum CandidateStyle {
    /// An upright style only.
    Normal,
    /// An italic style only.
    Italic,
    /// Both normal and italic styles (e.g. via an `ital` axis).
    NormalToItalic,
    /// A range of oblique angles in degrees (e.g. via a `slnt` axis).
    Oblique(AttrRange<f32>),
}

impl CandidateStyle {
    fn from_range(range: AttrRange<FontStyle>) -> Self {
        use FontStyle::*;
        let degrees = |angle: Option<f32>| angle.unwrap_or(DEFAULT_OBLIQUE_ANGLE);
        match (range.min(), range.max()) {
            (Normal, Normal) => Self::Normal,
            (Italic, Italic) => Self::Italic,
            (Normal, Italic) | (Italic, Normal) => Self::NormalToItalic,
            (Oblique(a), Oblique(b)) => {
                let (a, b) = (degrees(a), degrees(b));
                Self::Oblique(AttrRange::new(a.min(b), a.max(b)))
            }
            // A range spanning normal and oblique is interpreted as a range
            // of oblique angles extended to include upright (0deg).
            (Normal, Oblique(a)) | (Oblique(a), Normal) => {
                let a = degrees(a);
                Self::Oblique(AttrRange::new(a.min(0.), a.max(0.)))
            }
            // A range spanning italic and oblique has no meaningful
            // interpretation; treat it as italic.
            (Italic, Oblique(_)) | (Oblique(_), Italic) => Self::Italic,
        }
    }

    /// Returns `true` if the set of supported styles includes the given
    /// style.
    fn contains(self, style: FontStyle) -> bool {
        match style {
            FontStyle::Normal => self.contains_normal(),
            FontStyle::Italic => self.contains_italic(),
            FontStyle::Oblique(angle) => {
                let angle = angle.unwrap_or(DEFAULT_OBLIQUE_ANGLE);
                self.oblique().is_some_and(|range| range.contains(angle))
            }
        }
    }

    fn contains_normal(self) -> bool {
        match self {
            Self::Normal | Self::NormalToItalic => true,
            Self::Oblique(range) => range.contains(0.),
            Self::Italic => false,
        }
    }

    fn contains_italic(self) -> bool {
        matches!(self, Self::Italic | Self::NormalToItalic)
    }

    fn oblique(self) -> Option<AttrRange<f32>> {
        match self {
            Self::Oblique(range) => Some(range),
            _ => None,
        }
    }
}

fn match_candidates(
    mut set: CandidateVec,
    width: f32,
    style: FontStyle,
    weight: f32,
    synthesize_style: bool,
) -> Option<usize> {
    // font-width is tried first:
    if set.iter().any(|f| f.width.contains(width)) {
        // If the matching set includes faces with width values containing
        // the desired width, faces with width values which do not include
        // it are removed from the matching set.
        set.retain(|f| f.width.contains(width));
    } else {
        // Since no candidate's range contains the desired width, every
        // candidate lies entirely below or entirely above it. The closest
        // value below is a candidate's maximum and the closest value above
        // is a candidate's minimum.
        let below = set
            .iter()
            .map(|f| f.width.max())
            .filter(|w| *w < width)
            .max_by(f32::total_cmp);
        let above = set
            .iter()
            .map(|f| f.width.min())
            .filter(|w| *w > width)
            .min_by(f32::total_cmp);
        // If the desired width value is less than or equal to 100%, width
        // values below the desired width value are checked in descending
        // order followed by width values above the desired width value in
        // ascending order until a match is found. Otherwise, the search
        // is performed in the opposite direction.
        let use_width = if width <= 100.0 {
            below.or(above)
        } else {
            above.or(below)
        }?;
        // Faces with widths which do not include the determined width are
        // removed from the matching set.
        set.retain(|f| f.width.contains(use_width));
    }

    // font-style is tried next:
    retain_matching_style(&mut set, style, synthesize_style);

    // font-weight is matched next:
    if set.iter().any(|f| f.weight.contains(weight)) {
        // If the matching set includes faces with weight values containing
        // the desired weight, faces with weight values which do not include
        // it are removed from the matching set.
        set.retain(|f| f.weight.contains(weight));
    } else {
        // As with width, every candidate lies entirely below or entirely
        // above the desired weight at this point.
        let below = set
            .iter()
            .map(|f| f.weight.max())
            .filter(|w| *w < weight)
            .max_by(f32::total_cmp);
        let above = set
            .iter()
            .map(|f| f.weight.min())
            .filter(|w| *w > weight)
            .min_by(f32::total_cmp);
        // If the desired weight is inclusively between 400 and 500...
        let use_weight = if (400.0..=500.0).contains(&weight) {
            // weights greater than or equal to the target weight are checked
            // in ascending order until 500 is hit and checked...
            let above_to_500 = set
                .iter()
                .map(|f| f.weight.min())
                .filter(|w| *w > weight && *w <= 500.0)
                .min_by(f32::total_cmp);
            // followed by weights less than the target weight in descending
            // order, followed by weights greater than 500 in ascending
            // order, until a match is found.
            above_to_500.or(below).or(above)
        } else if weight < 400.0 {
            // If the desired weight is less than 400, weights less than or
            // equal to the target weight are checked in descending order,
            // followed by weights greater than the target weight in
            // ascending order.
            below.or(above)
        } else {
            // If the desired weight is greater than 500, weights greater
            // than or equal to the target weight are checked in ascending
            // order, followed by weights less than the target weight in
            // descending order.
            above.or(below)
        }?;
        set.retain(|f| f.weight.contains(use_weight));
    }

    set.first().map(|f| f.index)
}

/// Narrows the matching set to the faces that best match the desired style.
fn retain_matching_style(set: &mut CandidateVec, style: FontStyle, synthesize_style: bool) {
    // NOTE: this code uses an oblique threshold of 14deg rather than
    // the current value of 11deg in the spec.
    // See: https://github.com/w3c/csswg-drafts/issues/2295
    const OBLIQUE_THRESHOLD: f32 = DEFAULT_OBLIQUE_ANGLE;

    // If the matching set includes faces whose style range contains the
    // desired style, faces whose style range does not contain it are
    // removed from the matching set.
    if set.iter().any(|f| f.style.contains(style)) {
        set.retain(|f| f.style.contains(style));
        return;
    }
    match style {
        // If the value of font-style is italic:
        FontStyle::Italic => {
            // (Faces supporting italic would have been matched above.)
            // Oblique values greater than or equal to 14deg are checked in
            // ascending order,
            if let Some(found) = least_oblique_ge(set, OBLIQUE_THRESHOLD)
                // followed by positive oblique values below 14deg in
                // descending order.
                .or_else(|| greatest_oblique_pos_lt(set, OBLIQUE_THRESHOLD))
            {
                retain_oblique(set, found);
            }
            // If no match is found, normal faces are checked,
            else if set.iter().any(|f| f.style.contains_normal()) {
                set.retain(|f| f.style.contains_normal());
            }
            // followed by oblique values less than or equal to 0deg in
            // descending order.
            else if let Some(found) = greatest_oblique_le(set, 0.) {
                retain_oblique(set, found);
            }
        }
        // If the value of font-style is oblique:
        FontStyle::Oblique(angle) => {
            let angle = angle.unwrap_or(DEFAULT_OBLIQUE_ANGLE);
            let found = if angle >= OBLIQUE_THRESHOLD {
                // If the requested angle is greater than or equal to 14deg,
                // oblique values greater than or equal to the desired angle
                // are checked in ascending order, followed by positive
                // oblique values below the desired angle in descending
                // order.
                least_oblique_ge(set, angle).or_else(|| greatest_oblique_pos_lt(set, angle))
            } else if angle >= 0. {
                // If the requested angle is greater than or equal to 0deg
                // and less than 14deg, positive oblique values below the
                // desired angle are checked in descending order, followed
                // by oblique values above the desired angle in ascending
                // order.
                greatest_oblique_pos_lt(set, angle).or_else(|| least_oblique_ge(set, angle))
            } else if angle > -OBLIQUE_THRESHOLD {
                // If the requested angle is greater than -14deg and less
                // than 0deg, negative oblique values above the desired
                // angle are checked in ascending order, followed by oblique
                // values below the desired angle in descending order.
                least_oblique_neg_gt(set, angle).or_else(|| greatest_oblique_le(set, angle))
            } else {
                // If the requested angle is less than or equal to -14deg,
                // oblique values less than or equal to the desired angle
                // are checked in descending order, followed by negative
                // oblique values above the desired angle in ascending
                // order.
                greatest_oblique_le(set, angle).or_else(|| least_oblique_neg_gt(set, angle))
            };
            if let Some(found) = found {
                retain_oblique(set, found);
            }
            // If font-synthesis-style has the value auto, then for variable
            // fonts with a slnt axis a match is created by setting the slnt
            // value with the specified oblique value;
            else if synthesize_style && set.iter().any(|f| f.has_slnt) {
                set.retain(|f| f.has_slnt);
            }
            // otherwise, a fallback match is produced by geometric shearing
            // of an upright face to the specified oblique value.
            else if synthesize_style && set.iter().any(|f| f.style.contains_normal()) {
                set.retain(|f| f.style.contains_normal());
            }
            // If no match is found, italic faces are checked,
            else if set.iter().any(|f| f.style.contains_italic()) {
                set.retain(|f| f.style.contains_italic());
            }
            // followed by oblique values in the direction opposite to the
            // requested angle,
            else if let Some(found) = if angle >= 0. {
                // oblique values less than or equal to 0deg are checked in
                // descending order
                greatest_oblique_le(set, 0.)
            } else {
                // oblique values greater than or equal to 0deg are checked
                // in ascending order
                least_oblique_ge(set, 0.)
            } {
                retain_oblique(set, found);
            }
            // followed by normal faces.
            else if set.iter().any(|f| f.style.contains_normal()) {
                set.retain(|f| f.style.contains_normal());
            }
        }
        // If the value of font-style is normal:
        FontStyle::Normal => {
            // (Faces supporting normal would have been matched above.)
            // Oblique values greater than or equal to 0deg are checked in
            // ascending order,
            if let Some(found) = least_oblique_ge(set, 0.) {
                retain_oblique(set, found);
            }
            // followed by italic faces,
            else if set.iter().any(|f| f.style.contains_italic()) {
                set.retain(|f| f.style.contains_italic());
            }
            // followed by oblique values less than 0deg in descending order.
            else if let Some(found) = greatest_oblique_le(set, 0.) {
                retain_oblique(set, found);
            }
        }
    }
}

/// Removes the faces whose oblique range does not contain the given angle
/// from the matching set.
fn retain_oblique(set: &mut CandidateVec, angle: f32) {
    set.retain(|f| f.style.oblique().is_some_and(|range| range.contains(angle)));
}

/// Returns the least oblique angle greater than or equal to `angle`
/// supported by any face in the matching set.
fn least_oblique_ge(set: &[Candidate], angle: f32) -> Option<f32> {
    set.iter()
        .filter_map(|f| f.style.oblique())
        .filter(|range| range.max() >= angle)
        .map(|range| range.min().max(angle))
        .min_by(f32::total_cmp)
}

/// Returns the greatest positive oblique angle less than `angle` supported
/// by any face in the matching set.
fn greatest_oblique_pos_lt(set: &[Candidate], angle: f32) -> Option<f32> {
    set.iter()
        .filter_map(|f| f.style.oblique())
        .filter(|range| range.max() > 0. && range.min() < angle)
        .map(|range| range.max().min(angle))
        .max_by(f32::total_cmp)
}

/// Returns the greatest oblique angle less than or equal to `angle`
/// supported by any face in the matching set.
fn greatest_oblique_le(set: &[Candidate], angle: f32) -> Option<f32> {
    set.iter()
        .filter_map(|f| f.style.oblique())
        .filter(|range| range.min() <= angle)
        .map(|range| range.max().min(angle))
        .max_by(f32::total_cmp)
}

/// Returns the least negative oblique angle greater than `angle` supported
/// by any face in the matching set.
fn least_oblique_neg_gt(set: &[Candidate], angle: f32) -> Option<f32> {
    set.iter()
        .filter_map(|f| f.style.oblique())
        .filter(|range| range.min() > angle && range.min() < 0.)
        .map(|range| range.min())
        .min_by(f32::total_cmp)
}

#[cfg(test)]
mod tests {
    use super::*;

    const NORMAL: CandidateStyle = CandidateStyle::Normal;
    const ITALIC: CandidateStyle = CandidateStyle::Italic;

    fn oblique(min: f32, max: f32) -> CandidateStyle {
        CandidateStyle::Oblique(AttrRange::new(min, max))
    }

    /// A non-variable font supporting single width/style/weight values.
    fn static_font(index: usize, width: f32, style: CandidateStyle, weight: f32) -> Candidate {
        Candidate {
            index,
            width: AttrRange::single(width),
            style,
            weight: AttrRange::single(weight),
            has_slnt: false,
        }
    }

    /// A variable font supporting ranges of width/style/weight values.
    fn variable_font(
        index: usize,
        width: (f32, f32),
        style: CandidateStyle,
        weight: (f32, f32),
    ) -> Candidate {
        Candidate {
            index,
            width: AttrRange::new(width.0, width.1),
            style,
            weight: AttrRange::new(weight.0, weight.1),
            has_slnt: matches!(style, CandidateStyle::Oblique(_)),
        }
    }

    fn matched(set: &[Candidate], width: f32, style: FontStyle, weight: f32) -> Option<usize> {
        match_candidates(set.iter().copied().collect(), width, style, weight, false)
    }

    fn matched_synth(
        set: &[Candidate],
        width: f32,
        style: FontStyle,
        weight: f32,
    ) -> Option<usize> {
        match_candidates(set.iter().copied().collect(), width, style, weight, true)
    }

    #[test]
    fn weight_exact_match() {
        let set = [
            static_font(0, 100., NORMAL, 400.),
            static_font(1, 100., NORMAL, 700.),
        ];
        assert_eq!(matched(&set, 100., FontStyle::Normal, 400.), Some(0));
        assert_eq!(matched(&set, 100., FontStyle::Normal, 700.), Some(1));
    }

    #[test]
    fn weight_500_prefers_lower() {
        // For desired weights in 400..=500, weights above the desired weight
        // are only checked up to 500 before weights below.
        let set = [
            static_font(0, 100., NORMAL, 400.),
            static_font(1, 100., NORMAL, 700.),
        ];
        assert_eq!(matched(&set, 100., FontStyle::Normal, 500.), Some(0));
    }

    #[test]
    fn weight_450_prefers_up_to_500() {
        let set = [
            static_font(0, 100., NORMAL, 400.),
            static_font(1, 100., NORMAL, 480.),
        ];
        assert_eq!(matched(&set, 100., FontStyle::Normal, 450.), Some(1));
    }

    #[test]
    fn weight_below_400_prefers_lower() {
        let set = [
            static_font(0, 100., NORMAL, 200.),
            static_font(1, 100., NORMAL, 350.),
            static_font(2, 100., NORMAL, 400.),
        ];
        assert_eq!(matched(&set, 100., FontStyle::Normal, 300.), Some(0));
    }

    #[test]
    fn weight_above_500_prefers_higher() {
        let set = [
            static_font(0, 100., NORMAL, 550.),
            static_font(1, 100., NORMAL, 800.),
        ];
        assert_eq!(matched(&set, 100., FontStyle::Normal, 600.), Some(1));
    }

    #[test]
    fn weight_variable_range_contains() {
        // A variable font whose weight range contains the desired weight is
        // preferred over a static font with a closer single value.
        let set = [
            static_font(0, 100., NORMAL, 650.),
            variable_font(1, (100., 100.), NORMAL, (100., 600.)),
        ];
        assert_eq!(matched(&set, 100., FontStyle::Normal, 600.), Some(1));
        assert_eq!(matched(&set, 100., FontStyle::Normal, 500.), Some(1));
        assert_eq!(matched(&set, 100., FontStyle::Normal, 650.), Some(0));
        // 650 is closer to 700 than the variable font's range maximum (600).
        assert_eq!(matched(&set, 100., FontStyle::Normal, 700.), Some(0));
    }

    #[test]
    fn weight_range_boundaries() {
        let set = [
            variable_font(0, (100., 100.), NORMAL, (100., 300.)),
            variable_font(1, (100., 100.), NORMAL, (600., 900.)),
        ];
        // In 400..=500: check up to 500 (nothing), then below.
        assert_eq!(matched(&set, 100., FontStyle::Normal, 400.), Some(0));
        // Above 500: check above first.
        assert_eq!(matched(&set, 100., FontStyle::Normal, 550.), Some(1));
        // Below 400: check below first.
        assert_eq!(matched(&set, 100., FontStyle::Normal, 350.), Some(0));
    }

    #[test]
    fn width_exact_and_directional() {
        let set = [
            static_font(0, 75., NORMAL, 400.),
            static_font(1, 100., NORMAL, 400.),
            static_font(2, 125., NORMAL, 400.),
        ];
        assert_eq!(matched(&set, 100., FontStyle::Normal, 400.), Some(1));
        // Desired width <= 100%: prefer lower widths.
        assert_eq!(matched(&set, 90., FontStyle::Normal, 400.), Some(0));
        // Desired width > 100%: prefer higher widths.
        assert_eq!(matched(&set, 110., FontStyle::Normal, 400.), Some(2));
    }

    #[test]
    fn width_variable_range() {
        let set = [
            static_font(0, 100., NORMAL, 400.),
            variable_font(1, (50., 90.), NORMAL, (400., 400.)),
        ];
        // The variable font's width range contains the desired width.
        assert_eq!(matched(&set, 75., FontStyle::Normal, 400.), Some(1));
        // 95 is closer to the variable font's range maximum.
        assert_eq!(matched(&set, 95., FontStyle::Normal, 400.), Some(1));
        assert_eq!(matched(&set, 100., FontStyle::Normal, 400.), Some(0));
    }

    #[test]
    fn width_beats_style_and_weight() {
        // Width is matched before style and weight.
        let set = [
            static_font(0, 75., ITALIC, 700.),
            static_font(1, 100., NORMAL, 400.),
        ];
        assert_eq!(matched(&set, 100., FontStyle::Italic, 700.), Some(1));
    }

    #[test]
    fn style_exact_match() {
        let set = [
            static_font(0, 100., NORMAL, 400.),
            static_font(1, 100., ITALIC, 400.),
        ];
        assert_eq!(matched(&set, 100., FontStyle::Normal, 400.), Some(0));
        assert_eq!(matched(&set, 100., FontStyle::Italic, 400.), Some(1));
    }

    #[test]
    fn style_ital_axis_matches_both() {
        // A font with an `ital` axis spanning 0..=1 supports both normal
        // and italic.
        let set = [
            static_font(0, 100., ITALIC, 400.),
            variable_font(
                1,
                (100., 100.),
                CandidateStyle::NormalToItalic,
                (400., 400.),
            ),
        ];
        assert_eq!(matched(&set, 100., FontStyle::Normal, 400.), Some(1));
        assert_eq!(matched(&set, 100., FontStyle::Italic, 400.), Some(0));
    }

    #[test]
    fn style_slnt_axis_oblique_range() {
        // A font with a `slnt` axis supports a range of oblique angles.
        let set = [
            static_font(0, 100., NORMAL, 400.),
            variable_font(1, (100., 100.), oblique(0., 15.), (400., 400.)),
        ];
        // The oblique range contains the desired angle.
        assert_eq!(
            matched(&set, 100., FontStyle::Oblique(Some(10.)), 400.),
            Some(1)
        );
        assert_eq!(matched(&set, 100., FontStyle::Oblique(None), 400.), Some(1));
        // The oblique range contains 0deg, so it also matches normal, but
        // the upright font comes first in the set.
        assert_eq!(matched(&set, 100., FontStyle::Normal, 400.), Some(0));
    }

    #[test]
    fn style_italic_falls_back_to_oblique() {
        let set = [
            static_font(0, 100., NORMAL, 400.),
            variable_font(1, (100., 100.), oblique(0., 15.), (400., 400.)),
        ];
        // No italic face: obliques >= 14deg are preferred.
        assert_eq!(matched(&set, 100., FontStyle::Italic, 400.), Some(1));
    }

    #[test]
    fn style_italic_prefers_larger_oblique() {
        let set = [
            static_font(0, 100., oblique(5., 5.), 400.),
            static_font(1, 100., oblique(20., 20.), 400.),
        ];
        // Obliques >= 14deg are checked (ascending) before positive
        // obliques below 14deg (descending).
        assert_eq!(matched(&set, 100., FontStyle::Italic, 400.), Some(1));
    }

    #[test]
    fn style_oblique_prefers_italic_over_normal() {
        let set = [
            static_font(0, 100., NORMAL, 400.),
            static_font(1, 100., ITALIC, 400.),
        ];
        // Without synthesis, italic is preferred over normal for oblique
        // requests.
        assert_eq!(matched(&set, 100., FontStyle::Oblique(None), 400.), Some(1));
        // With synthesis, an upright face is shearable, so normal is
        // preferred.
        assert_eq!(
            matched_synth(&set, 100., FontStyle::Oblique(None), 400.),
            Some(0)
        );
    }

    #[test]
    fn style_oblique_synthesis_prefers_slnt_axis() {
        let mut slanted = static_font(1, 100., NORMAL, 400.);
        slanted.has_slnt = true;
        let set = [static_font(0, 100., NORMAL, 400.), slanted];
        // With synthesis enabled, fonts with a `slnt` axis are preferred
        // for oblique requests.
        assert_eq!(
            matched_synth(&set, 100., FontStyle::Oblique(Some(20.)), 400.),
            Some(1)
        );
    }

    #[test]
    fn style_oblique_above_threshold_prefers_ascending() {
        let set = [
            static_font(0, 100., oblique(5., 10.), 400.),
            static_font(1, 100., oblique(30., 40.), 400.),
        ];
        // For angles >= 14deg, larger angles are checked first.
        assert_eq!(
            matched(&set, 100., FontStyle::Oblique(Some(20.)), 400.),
            Some(1)
        );
        // For positive angles < 14deg, smaller angles are checked first.
        assert_eq!(
            matched(&set, 100., FontStyle::Oblique(Some(12.)), 400.),
            Some(0)
        );
    }

    #[test]
    fn style_negative_oblique() {
        let set = [
            static_font(0, 100., oblique(-40., -30.), 400.),
            static_font(1, 100., oblique(-10., -5.), 400.),
        ];
        // The range containing the desired angle matches.
        assert_eq!(
            matched(&set, 100., FontStyle::Oblique(Some(-35.)), 400.),
            Some(0)
        );
        // For angles <= -14deg, more negative angles are checked first.
        assert_eq!(
            matched(&set, 100., FontStyle::Oblique(Some(-20.)), 400.),
            Some(0)
        );
        // For negative angles > -14deg, less negative angles are checked
        // first.
        assert_eq!(
            matched(&set, 100., FontStyle::Oblique(Some(-2.)), 400.),
            Some(1)
        );
    }

    #[test]
    fn style_normal_prefers_smallest_oblique() {
        let set = [
            static_font(0, 100., ITALIC, 400.),
            static_font(1, 100., oblique(10., 20.), 400.),
        ];
        // No face supports normal: obliques >= 0deg are preferred over
        // italic.
        assert_eq!(matched(&set, 100., FontStyle::Normal, 400.), Some(1));
    }

    #[test]
    fn empty_set() {
        assert_eq!(matched(&[], 100., FontStyle::Normal, 400.), None);
    }
}
