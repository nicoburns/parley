// Copyright 2026 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Tests for attribute range handling with real variable fonts.

use std::sync::Arc;

use fontique::{
    AttrRange, Blob, Collection, CollectionOptions, FontInfoOverride, FontStyle, FontWeight,
    FontWidth,
};

fn collection() -> Collection {
    Collection::new(CollectionOptions {
        shared: false,
        system_fonts: false,
    })
}

fn roboto_flex_data() -> Blob<u8> {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../parley_dev/assets/fonts/roboto_fonts/RobotoFlex-VariableFont.ttf"
    );
    Blob::new(Arc::new(std::fs::read(path).unwrap()))
}

fn roboto_regular_data() -> Blob<u8> {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../parley_dev/assets/fonts/roboto_fonts/Roboto-Regular.ttf"
    );
    Blob::new(Arc::new(std::fs::read(path).unwrap()))
}

#[test]
fn variable_font_attribute_ranges() {
    // Roboto Flex has (amongst others) the following axes:
    //   wght: 100..=1000 (default 400)
    //   wdth: 25..=151 (default 100)
    //   slnt: -10..=0 (default 0)
    let mut collection = collection();
    let ids = collection.register_fonts(roboto_flex_data(), None);
    assert_eq!(ids.len(), 1);

    let family = collection.family(ids[0].family()).unwrap();
    let font = &family.fonts()[0];
    assert_eq!(
        font.weight(),
        AttrRange::new(FontWeight::new(100.), FontWeight::new(1000.))
    );
    assert_eq!(
        font.width(),
        AttrRange::new(
            FontWidth::from_percentage(25.),
            FontWidth::from_percentage(151.)
        )
    );
    // The `slnt` axis range is negated when mapped to CSS oblique angles.
    assert_eq!(
        font.style(),
        AttrRange::new(FontStyle::Oblique(Some(0.)), FontStyle::Oblique(Some(10.)))
    );
}

#[test]
fn variable_font_matching_and_synthesis() {
    let mut collection = collection();
    let ids = collection.register_fonts(roboto_flex_data(), None);
    let family = collection.family(ids[0].family()).unwrap();

    // Weights and widths within the variable ranges match the font, and
    // synthesis suggests the corresponding variation settings.
    let font = family
        .match_font(
            FontWidth::CONDENSED,
            FontStyle::Oblique(Some(5.)),
            FontWeight::BOLD,
            false,
        )
        .unwrap();
    let synthesis = font.synthesis(
        FontWidth::CONDENSED,
        FontStyle::Oblique(Some(5.)),
        FontWeight::BOLD,
    );
    let vars: Vec<_> = synthesis
        .variation_settings()
        .iter()
        .map(|(tag, value)| (tag.to_be_bytes(), *value))
        .collect();
    assert!(vars.contains(&(*b"wdth", 75.)));
    assert!(vars.contains(&(*b"wght", 700.)));
    // CSS oblique angles are negated when applied to the `slnt` axis.
    assert!(vars.contains(&(*b"slnt", -5.)));

    // Values outside the variable ranges are clamped.
    let synthesis = font.synthesis(
        FontWidth::NORMAL,
        FontStyle::Oblique(Some(20.)),
        FontWeight::new(1200.),
    );
    let vars: Vec<_> = synthesis
        .variation_settings()
        .iter()
        .map(|(tag, value)| (tag.to_be_bytes(), *value))
        .collect();
    assert!(vars.contains(&(*b"wght", 1000.)));
    assert!(vars.contains(&(*b"slnt", -10.)));

    // The default instance requires no synthesis.
    let synthesis = font.synthesis(FontWidth::NORMAL, FontStyle::Normal, FontWeight::NORMAL);
    assert!(!synthesis.any());
}

#[test]
fn static_font_attribute_ranges() {
    let mut collection = collection();
    let ids = collection.register_fonts(roboto_regular_data(), None);
    assert_eq!(ids.len(), 1);

    let family = collection.family(ids[0].family()).unwrap();
    let font = &family.fonts()[0];
    assert!(font.width().is_single());
    assert!(font.style().is_single());
    assert!(font.weight().is_single());
    assert_eq!(font.width().min(), FontWidth::NORMAL);
    assert_eq!(font.style().min(), FontStyle::Normal);
    assert_eq!(font.weight().min(), FontWeight::NORMAL);
}

#[test]
fn unregister_font_by_id() {
    let mut collection = collection();
    let ids = collection.register_fonts(roboto_regular_data(), None);
    assert_eq!(ids.len(), 1);
    let family_id = ids[0].family();
    assert_eq!(collection.family(family_id).unwrap().fonts().len(), 1);

    assert!(collection.unregister_font(ids[0]));
    assert!(collection.family(family_id).unwrap().fonts().is_empty());
    // Unregistering again removes nothing.
    assert!(!collection.unregister_font(ids[0]));
}

#[test]
fn override_with_range() {
    let mut collection = collection();
    let ids = collection.register_fonts(
        roboto_regular_data(),
        Some(FontInfoOverride {
            weight: Some(AttrRange::new(FontWeight::new(300.), FontWeight::new(800.))),
            width: Some(FontWidth::CONDENSED.into()),
            ..Default::default()
        }),
    );
    let family = collection.family(ids[0].family()).unwrap();
    let font = &family.fonts()[0];
    assert_eq!(
        font.weight(),
        AttrRange::new(FontWeight::new(300.), FontWeight::new(800.))
    );
    assert_eq!(font.width(), AttrRange::single(FontWidth::CONDENSED));
}
