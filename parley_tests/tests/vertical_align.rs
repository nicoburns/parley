// Copyright 2026 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Vertical alignment tests.
//!
//! These tests validate that the `AlignmentBaseline` and `BaselineShift` styles
//! affect line metrics and the vertical position of glyph runs.

use crate::test_name;
use crate::util::{ColorBrush, TestEnv};
use parley::{BaselineShift, Layout, PositionedLayoutItem, StyleProperty};

const FONT_SIZE: f32 = 32.0;

/// Builds a single (unwrapped) line layout for `text` using `configure` to add styles.
fn build_single_line(
    env: &mut TestEnv,
    text: &str,
    configure: impl FnOnce(&mut parley::RangedBuilder<'_, ColorBrush>),
) -> Layout<ColorBrush> {
    let mut builder = env.ranged_builder(text);
    builder.push_default(StyleProperty::FontSize(FONT_SIZE));
    configure(&mut builder);
    let mut layout = builder.build(text);
    layout.break_all_lines(None);
    layout
}

/// Returns the ascent of the first line.
fn first_line_ascent(layout: &Layout<ColorBrush>) -> f32 {
    layout.lines().next().unwrap().metrics().ascent
}

/// Returns the baselines of all glyph runs in the layout, in visual order.
fn glyph_run_baselines(layout: &Layout<ColorBrush>) -> Vec<f32> {
    let mut baselines = Vec::new();
    for line in layout.lines() {
        for item in line.items() {
            if let PositionedLayoutItem::GlyphRun(run) = item {
                baselines.push(run.baseline());
            }
        }
    }
    baselines
}

#[test]
fn vertical_align_default_keeps_single_baseline() {
    let mut env = TestEnv::new(test_name!(), None);
    let layout = build_single_line(&mut env, "normal text", |_| {});

    let baselines = glyph_run_baselines(&layout);
    assert!(!baselines.is_empty(), "expected at least one glyph run");
    for baseline in &baselines {
        assert_eq!(
            *baseline, baselines[0],
            "expected all default runs to share one baseline"
        );
    }
}

#[test]
fn vertical_align_superscript_raises_partial_run() {
    let mut env = TestEnv::new(test_name!(), None);
    let text = "base super";
    let super_start = text.find("super").unwrap();
    let layout = build_single_line(&mut env, text, |builder| {
        builder.push(
            StyleProperty::BaselineShift(BaselineShift::Superscript),
            super_start..text.len(),
        );
    });

    let baselines = glyph_run_baselines(&layout);
    let min = baselines.iter().copied().fold(f32::INFINITY, f32::min);
    let max = baselines.iter().copied().fold(f32::NEG_INFINITY, f32::max);
    assert!(
        min < max,
        "expected superscript span to sit above the baseline ({min} < {max})"
    );
}

#[test]
fn vertical_align_subscript_lowers_partial_run() {
    let mut env = TestEnv::new(test_name!(), None);
    let text = "base sub";
    let sub_start = text.find("sub").unwrap();
    let layout = build_single_line(&mut env, text, |builder| {
        builder.push(
            StyleProperty::BaselineShift(BaselineShift::Subscript),
            sub_start..text.len(),
        );
    });

    let baselines = glyph_run_baselines(&layout);
    let min = baselines.iter().copied().fold(f32::INFINITY, f32::min);
    let max = baselines.iter().copied().fold(f32::NEG_INFINITY, f32::max);
    assert!(
        max > min,
        "expected subscript span to sit below the baseline ({max} > {min})"
    );
}

#[test]
fn vertical_align_superscript_grows_line_ascent() {
    let mut env = TestEnv::new(test_name!(), None);

    let plain = build_single_line(&mut env, "super", |_| {});
    let plain_ascent = first_line_ascent(&plain);

    let shifted = build_single_line(&mut env, "super", |builder| {
        builder.push_default(StyleProperty::BaselineShift(BaselineShift::Superscript));
    });
    let shifted_ascent = first_line_ascent(&shifted);

    assert!(
        shifted_ascent > plain_ascent,
        "expected superscript to grow line ascent ({shifted_ascent} > {plain_ascent})"
    );
}

#[test]
fn vertical_align_absolute_shift_grows_line_descent() {
    let mut env = TestEnv::new(test_name!(), None);

    let plain = build_single_line(&mut env, "shift", |_| {});
    let plain_descent = plain.lines().next().unwrap().metrics().descent;

    // A negative absolute shift moves the baseline down (y-down), growing descent.
    let shifted = build_single_line(&mut env, "shift", |builder| {
        builder.push_default(StyleProperty::BaselineShift(BaselineShift::Absolute(-10.0)));
    });
    let shifted_descent = shifted.lines().next().unwrap().metrics().descent;

    assert!(
        shifted_descent > plain_descent,
        "expected downward shift to grow line descent ({shifted_descent} > {plain_descent})"
    );
}
