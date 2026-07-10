// Copyright 2021 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Hierarchical tree based style application.
use alloc::{string::String, vec::Vec};

use crate::style::WhiteSpaceCollapse;

use super::{Brush, ResolvedProperty, ResolvedStyle, StyleRun};

#[derive(Debug, Clone)]
struct StyleTreeNode<B: Brush> {
    parent: Option<usize>,
    style: ResolvedStyle<B>,
    style_id: Option<u16>,
}

#[derive(Clone, Copy, PartialEq)]
pub(crate) enum ItemKind {
    None,
    InlineBox,
    TextRun,
}

/// Builder for constructing a tree of styles
#[derive(Clone)]
pub(crate) struct TreeStyleBuilder<B: Brush> {
    tree: Vec<StyleTreeNode<B>>,
    style_table: Vec<ResolvedStyle<B>>,
    style_runs: Vec<StyleRun>,
    white_space_collapse: WhiteSpaceCollapse,
    text: String,
    uncommitted_text: String,
    current_span: usize,
    last_item_kind: ItemKind,
}

impl<B: Brush> TreeStyleBuilder<B> {
    fn current_style(&self) -> ResolvedStyle<B> {
        self.tree[self.current_span].style.clone()
    }
}

impl<B: Brush> Default for TreeStyleBuilder<B> {
    fn default() -> Self {
        Self {
            tree: Vec::new(),
            style_table: Vec::new(),
            style_runs: Vec::new(),
            white_space_collapse: WhiteSpaceCollapse::Preserve,
            text: String::new(),
            uncommitted_text: String::new(),
            current_span: usize::MAX,
            last_item_kind: ItemKind::None,
        }
    }
}

impl<B: Brush> TreeStyleBuilder<B> {
    /// Prepares the builder for accepting a tree of styles and text.
    ///
    /// The provided `root_style` is the default style applied to all text unless overridden.
    pub(crate) fn begin(&mut self, root_style: ResolvedStyle<B>) {
        self.tree.clear();
        self.style_table.clear();
        self.style_runs.clear();
        self.white_space_collapse = WhiteSpaceCollapse::Preserve;
        self.text.clear();
        self.uncommitted_text.clear();

        self.tree.push(StyleTreeNode {
            parent: None,
            style: root_style,
            style_id: None,
        });
        self.current_span = 0;
        self.last_item_kind = ItemKind::None;
    }

    pub(crate) fn set_white_space_mode(&mut self, white_space_collapse: WhiteSpaceCollapse) {
        self.white_space_collapse = white_space_collapse;
    }

    pub(crate) fn set_last_item_kind(&mut self, item_kind: ItemKind) {
        self.last_item_kind = item_kind;
    }

    pub(crate) fn push_uncommitted_text(&mut self, is_layout_end: bool) {
        let uncommitted_text = core::mem::take(&mut self.uncommitted_text);
        let span_text = match self.white_space_collapse {
            WhiteSpaceCollapse::Preserve => uncommitted_text,
            WhiteSpaceCollapse::Collapse => {
                let mut span_text = uncommitted_text.as_str();

                // Collapsing operates across the whole layout: style span boundaries neither
                // block collapsing nor cause trimming. White space is trimmed at the very start
                // of the layout, and collapsed away entirely when the preceding text run already
                // ends with white space (but preserved after an inline box).
                let trim_start = match self.last_item_kind {
                    ItemKind::None => true,
                    ItemKind::InlineBox => false,
                    ItemKind::TextRun => self
                        .text
                        .chars()
                        .last()
                        .is_some_and(|c| c.is_ascii_whitespace()),
                };
                if trim_start {
                    span_text = span_text.trim_start();
                }
                if is_layout_end {
                    span_text = span_text.trim_end();
                }

                // Collapse spaces
                let mut last_char_whitespace = false;
                span_text
                    .chars()
                    .filter_map(|c: char| {
                        let this_char_whitespace = c.is_ascii_whitespace();
                        let prev_char_whitespace = last_char_whitespace;
                        last_char_whitespace = this_char_whitespace;

                        if this_char_whitespace {
                            if prev_char_whitespace {
                                None
                            } else {
                                Some(' ')
                            }
                        } else {
                            Some(c)
                        }
                    })
                    .collect()
            }
        };

        // Nothing to do if there is no uncommitted text.
        if span_text.is_empty() {
            return;
        }

        let range = self.text.len()..(self.text.len() + span_text.len());
        let style_index = self.resolve_current_style_id();
        self.style_runs.push(StyleRun { style_index, range });
        self.text.push_str(&span_text);
        self.last_item_kind = ItemKind::TextRun;
    }

    fn resolve_current_style_id(&mut self) -> u16 {
        if let Some(style_id) = self.tree[self.current_span].style_id {
            return style_id;
        }
        let style_id = self.style_table.len() as u16;
        self.style_table.push(self.current_style());
        self.tree[self.current_span].style_id = Some(style_id);
        style_id
    }

    pub(crate) fn current_text_len(&self) -> usize {
        self.text.len()
    }

    pub(crate) fn push_style_span(&mut self, style: ResolvedStyle<B>) {
        self.push_uncommitted_text(false);

        self.tree.push(StyleTreeNode {
            parent: Some(self.current_span),
            style,
            style_id: None,
        });
        self.current_span = self.tree.len() - 1;
    }

    pub(crate) fn push_style_modification_span(
        &mut self,
        properties: impl Iterator<Item = ResolvedProperty<B>>,
    ) {
        let mut style = self.current_style();
        for prop in properties {
            style.apply(prop.clone());
        }
        self.push_style_span(style);
    }

    pub(crate) fn pop_style_span(&mut self) {
        self.push_uncommitted_text(false);

        self.current_span = self.tree[self.current_span]
            .parent
            .expect("Popped root style");
    }

    /// Pushes a property that covers the specified range of text.
    pub(crate) fn push_text(&mut self, text: &str) {
        if !text.is_empty() {
            self.uncommitted_text.push_str(text);
        }
    }

    /// Computes style table + style runs and returns the final text buffer.
    pub(crate) fn finish(
        &mut self,
        style_table: &mut Vec<ResolvedStyle<B>>,
        style_runs: &mut Vec<StyleRun>,
    ) -> String {
        // Commit any remaining text first: it is the last text of the layout, so trailing white
        // space is trimmed (`is_layout_end`) even when the text sits inside an unclosed span.
        self.push_uncommitted_text(true);

        while self.tree[self.current_span].parent.is_some() {
            self.pop_style_span();
        }

        style_table.clear();
        style_runs.clear();
        style_table.extend_from_slice(&self.style_table);
        style_runs.extend_from_slice(&self.style_runs);

        core::mem::take(&mut self.text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec::Vec;
    use core::ops::Range;

    #[test]
    fn reuses_style_id_when_returning_to_parent_span() {
        let mut builder = TreeStyleBuilder::<u32>::default();
        builder.begin(ResolvedStyle::default());
        builder.push_text("A");
        builder.push_style_modification_span([ResolvedProperty::FontSize(20.)].into_iter());
        builder.push_text("B");
        builder.pop_style_span();
        builder.push_text("C");

        let mut style_table = Vec::new();
        let mut style_runs = Vec::new();
        let text = builder.finish(&mut style_table, &mut style_runs);

        assert_eq!(text, "ABC");
        assert_eq!(style_table.len(), 2);
        assert_eq!(style_runs.len(), 3);
        assert_eq!(style_runs[0].style_index, 0);
        assert_eq!(style_runs[1].style_index, 1);
        assert_eq!(style_runs[2].style_index, 0);
        assert_eq!(style_runs[0].range, Range { start: 0, end: 1 });
        assert_eq!(style_runs[1].range, Range { start: 1, end: 2 });
        assert_eq!(style_runs[2].range, Range { start: 2, end: 3 });
    }

    #[test]
    fn reuses_root_style_id_across_multiple_pop_return_cycles() {
        let mut builder = TreeStyleBuilder::<u32>::default();
        builder.begin(ResolvedStyle::default());
        builder.push_text("A");
        builder.push_style_modification_span([ResolvedProperty::FontSize(20.)].into_iter());
        builder.push_text("B");
        builder.pop_style_span();
        builder.push_text("C");
        builder.push_style_modification_span([ResolvedProperty::LetterSpacing(1.)].into_iter());
        builder.push_text("D");
        builder.pop_style_span();
        builder.push_text("E");

        let mut style_table = Vec::new();
        let mut style_runs = Vec::new();
        let text = builder.finish(&mut style_table, &mut style_runs);

        assert_eq!(text, "ABCDE");
        assert_eq!(style_table.len(), 3);
        assert_eq!(style_runs.len(), 5);
        assert_eq!(style_runs[0].style_index, 0);
        assert_eq!(style_runs[1].style_index, 1);
        assert_eq!(style_runs[2].style_index, 0);
        assert_eq!(style_runs[3].style_index, 2);
        assert_eq!(style_runs[4].style_index, 0);
    }

    #[test]
    fn reuses_parent_and_root_style_ids_after_nested_pop() {
        let mut builder = TreeStyleBuilder::<u32>::default();
        builder.begin(ResolvedStyle::default());
        builder.push_text("R");
        builder.push_style_modification_span([ResolvedProperty::FontSize(20.)].into_iter());
        builder.push_text("A");
        builder.push_style_modification_span([ResolvedProperty::LetterSpacing(1.)].into_iter());
        builder.push_text("B");
        builder.pop_style_span();
        builder.push_text("C");
        builder.pop_style_span();
        builder.push_text("D");

        let mut style_table = Vec::new();
        let mut style_runs = Vec::new();
        let text = builder.finish(&mut style_table, &mut style_runs);

        assert_eq!(text, "RABCD");
        assert_eq!(style_table.len(), 3);
        assert_eq!(style_runs.len(), 5);
        assert_eq!(style_runs[0].style_index, 0);
        assert_eq!(style_runs[1].style_index, 1);
        assert_eq!(style_runs[2].style_index, 2);
        assert_eq!(style_runs[3].style_index, 1);
        assert_eq!(style_runs[4].style_index, 0);
    }

    /// Helper that runs `texts` through the tree builder, each text in its own child span
    /// (`None` entries are pushed as text directly at the root), with the given
    /// white-space-collapse mode, and returns the resulting text buffer.
    fn collapsed_spans(mode: WhiteSpaceCollapse, texts: &[(Option<()>, &str)]) -> String {
        let mut builder = TreeStyleBuilder::<u32>::default();
        builder.begin(ResolvedStyle::default());
        builder.set_white_space_mode(mode);
        for (span, text) in texts {
            if span.is_some() {
                builder.push_style_modification_span([ResolvedProperty::FontSize(20.)].into_iter());
                builder.push_text(text);
                builder.pop_style_span();
            } else {
                builder.push_text(text);
            }
        }
        let mut style_table = Vec::new();
        let mut style_runs = Vec::new();
        builder.finish(&mut style_table, &mut style_runs)
    }

    /// White space collapsing operates across the whole inline formatting context: span
    /// boundaries neither block collapsing nor cause trimming. Trimming only happens at the
    /// boundaries of the layout as a whole.
    #[test]
    fn collapse_preserves_white_space_at_span_boundaries() {
        use WhiteSpaceCollapse::*;
        let span = Some(());

        // Trailing white space at the end of a span collapses with what follows,
        // leaving a single space (e.g. `<span>foo </span>bar`).
        assert_eq!(
            collapsed_spans(Collapse, &[(span, "foo "), (None, "bar")]),
            "foo bar"
        );
        // Leading white space at the start of a span collapses with what precedes it
        // (e.g. `foo<span> bar</span>`).
        assert_eq!(
            collapsed_spans(Collapse, &[(None, "foo"), (span, " bar")]),
            "foo bar"
        );
        // White space on both sides of a span boundary collapses to a single space.
        assert_eq!(
            collapsed_spans(Collapse, &[(None, "foo "), (span, " bar")]),
            "foo bar"
        );
        // A white-space-only span between two words collapses to a single space.
        assert_eq!(
            collapsed_spans(Collapse, &[(None, "foo"), (span, " "), (None, "bar")]),
            "foo bar"
        );
        // White space at the very start of the layout is trimmed, even inside a span.
        assert_eq!(
            collapsed_spans(Collapse, &[(span, " foo"), (None, " bar")]),
            "foo bar"
        );
        // A white-space-only span at the start of the layout is trimmed entirely.
        assert_eq!(
            collapsed_spans(Collapse, &[(span, " "), (None, "foo")]),
            "foo"
        );
    }
}
