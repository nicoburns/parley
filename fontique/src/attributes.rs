// Copyright 2024 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Properties for specifying font matching attributes.

use core::fmt;

use parlance::{FontStyle, FontWeight, FontWidth};

/// Primary attributes for font matching: [`FontWidth`], [`FontStyle`] and [`FontWeight`].
///
/// These are used to [configure] a [`Query`].
///
/// [configure]: crate::Query::set_attributes
/// [`Query`]: crate::Query
#[derive(Copy, Clone, PartialEq, Default, Debug)]
pub struct Attributes {
    pub width: FontWidth,
    pub style: FontStyle,
    pub weight: FontWeight,
}

impl Attributes {
    /// Creates new attributes from the given width, style and weight.
    pub fn new(width: FontWidth, style: FontStyle, weight: FontWeight) -> Self {
        Self {
            width,
            style,
            weight,
        }
    }
}

impl fmt::Display for Attributes {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "width: {}, style: {}, weight: {}",
            self.width, self.style, self.weight
        )
    }
}

/// An inclusive range of values for a font attribute such as [`FontWidth`],
/// [`FontStyle`] or [`FontWeight`].
///
/// Traditional (non-variable) fonts support a single value for each attribute,
/// which is represented by a trivial range where `min == max` (see
/// [`AttrRange::single`]). Variable fonts may support a continuous range of
/// values, derived from the corresponding variation axis (e.g. `wght` for
/// weight).
#[derive(Copy, Clone, PartialEq, Debug)]
pub struct AttrRange<T> {
    min: T,
    max: T,
}

impl<T> AttrRange<T> {
    /// Creates a new range with the given minimum and maximum values (both
    /// inclusive).
    pub const fn new(min: T, max: T) -> Self {
        Self { min, max }
    }

    /// Creates a trivial range containing the single given value.
    pub const fn single(value: T) -> Self
    where
        T: Copy,
    {
        Self {
            min: value,
            max: value,
        }
    }

    /// Returns the minimum value of the range.
    pub fn min(&self) -> T
    where
        T: Copy,
    {
        self.min
    }

    /// Returns the maximum value of the range.
    pub fn max(&self) -> T
    where
        T: Copy,
    {
        self.max
    }

    /// Returns `true` if the range contains a single value.
    pub fn is_single(&self) -> bool
    where
        T: PartialEq,
    {
        self.min == self.max
    }

    /// Returns `true` if the range contains the given value.
    pub fn contains(&self, value: T) -> bool
    where
        T: PartialOrd,
    {
        self.min <= value && value <= self.max
    }

    /// Clamps the given value into the range.
    pub fn clamp(&self, value: T) -> T
    where
        T: Copy + PartialOrd,
    {
        if value < self.min {
            self.min
        } else if value > self.max {
            self.max
        } else {
            value
        }
    }
}

impl<T: Copy> From<T> for AttrRange<T> {
    fn from(value: T) -> Self {
        Self::single(value)
    }
}

impl<T: Copy> From<(T, T)> for AttrRange<T> {
    fn from((min, max): (T, T)) -> Self {
        Self::new(min, max)
    }
}

impl<T: fmt::Display + PartialEq> fmt::Display for AttrRange<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_single() {
            write!(f, "{}", self.min)
        } else {
            write!(f, "{}..={}", self.min, self.max)
        }
    }
}
