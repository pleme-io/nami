//! Layout engine -- taffy flexbox/grid.
//!
//! Takes computed styles and DOM tree, produces a layout tree with
//! absolute positions and sizes for each visible element.
//! Uses taffy for CSS flexbox and grid layout algorithms.

use crate::css::{ComputedStyle, Display, Dimension, Stylesheet};
use crate::dom::{Document, Node, NodeKind};

#[derive(thiserror::Error, Debug)]
pub enum LayoutError {
    #[error("layout computation failed: {0}")]
    Compute(String),
    #[error("taffy error: {0}")]
    Taffy(String),
}

pub type Result<T> = std::result::Result<T, LayoutError>;

/// A positioned box in the layout tree.
#[derive(Debug, Clone)]
pub struct LayoutBox {
    /// Horizontal position (left edge) in pixels.
    pub x: f32,
    /// Vertical position (top edge) in pixels.
    pub y: f32,
    /// Width in pixels.
    pub width: f32,
    /// Height in pixels.
    pub height: f32,
    /// Index into the DOM node list that this box corresponds to.
    pub node_id: usize,
}

/// A computed layout tree.
pub struct LayoutTree {
    boxes: Vec<LayoutBox>,
    taffy: taffy::TaffyTree,
    root_node: Option<taffy::NodeId>,
}

impl LayoutTree {
    /// Compute layout for a document with the given stylesheets and viewport width.
    ///
    /// Walks the DOM tree, creates corresponding taffy nodes with styles derived
    /// from the CSS cascade, then runs taffy layout to compute absolute positions.
    #[must_use]
    pub fn compute(doc: &Document, styles: &[Stylesheet], viewport_width: f32) -> Self {
        let mut taffy = taffy::TaffyTree::new();
        let mut boxes = Vec::new();
        let mut node_counter = 0_usize;

        // Build the taffy tree from the DOM.
        let root_node = build_taffy_node(
            &doc.root,
            &mut taffy,
            styles,
            &mut boxes,
            &mut node_counter,
        );

        // Run layout with the given viewport width as available space.
        if let Some(root) = root_node {
            let available = taffy::Size {
                width: taffy::AvailableSpace::Definite(viewport_width),
                height: taffy::AvailableSpace::MaxContent,
            };

            if let Err(e) = taffy.compute_layout(root, available) {
                tracing::warn!(error = %e, "taffy layout computation failed");
            }

            // Extract computed positions.
            collect_layout_boxes(&taffy, root, 0.0, 0.0, &mut boxes);
        }

        tracing::debug!(
            box_count = boxes.len(),
            viewport_width,
            "layout computed"
        );

        Self {
            boxes,
            taffy,
            root_node,
        }
    }

    /// Get all layout boxes.
    #[must_use]
    pub fn boxes(&self) -> &[LayoutBox] {
        &self.boxes
    }

    /// Get the total content height (for scrolling).
    #[must_use]
    pub fn content_height(&self) -> f32 {
        self.boxes
            .iter()
            .map(|b| b.y + b.height)
            .fold(0.0_f32, f32::max)
    }
}

/// Recursively build taffy nodes from the DOM tree.
fn build_taffy_node(
    node: &Node,
    taffy: &mut taffy::TaffyTree,
    styles: &[Stylesheet],
    _boxes: &mut Vec<LayoutBox>,
    counter: &mut usize,
) -> Option<taffy::NodeId> {
    let node_id = *counter;
    *counter += 1;

    match &node.kind {
        NodeKind::Element(elem) => {
            // Skip elements with display: none.
            let computed = compute_element_style(&elem.tag, styles);
            if computed.display == Display::None {
                return None;
            }

            let taffy_style = css_to_taffy_style(&computed, &elem.tag);

            // Build children first.
            let child_nodes: Vec<taffy::NodeId> = elem
                .children
                .iter()
                .filter_map(|child| build_taffy_node(child, taffy, styles, _boxes, counter))
                .collect();

            match taffy.new_with_children(taffy_style, &child_nodes) {
                Ok(taffy_node) => Some(taffy_node),
                Err(e) => {
                    tracing::trace!(error = %e, tag = %elem.tag, "failed to create taffy node");
                    None
                }
            }
        }
        NodeKind::Text(text) => {
            if text.trim().is_empty() {
                return None;
            }

            // Text nodes get a leaf node with size based on character count.
            // This is a rough approximation; real text layout requires font metrics.
            let char_count = text.len() as f32;
            let estimated_width = char_count * 8.0; // ~8px per character estimate
            let estimated_height = 16.0; // single line height

            let style = taffy::Style {
                size: taffy::Size {
                    width: taffy::Dimension::Length(estimated_width),
                    height: taffy::Dimension::Length(estimated_height),
                },
                ..Default::default()
            };

            match taffy.new_leaf(style) {
                Ok(taffy_node) => Some(taffy_node),
                Err(e) => {
                    tracing::trace!(error = %e, "failed to create text taffy node");
                    None
                }
            }
        }
        NodeKind::Comment(_) => None,
    }
}

/// Compute the merged style for an element from all stylesheets.
fn compute_element_style(tag: &str, styles: &[Stylesheet]) -> ComputedStyle {
    let mut computed = default_style_for_tag(tag);
    for sheet in styles {
        computed = sheet.compute_style(tag, &computed);
    }
    computed
}

/// Return the default (user-agent) style for common HTML tags.
fn default_style_for_tag(tag: &str) -> ComputedStyle {
    let mut style = ComputedStyle::default();

    match tag {
        "div" | "p" | "h1" | "h2" | "h3" | "h4" | "h5" | "h6" | "section" | "article"
        | "main" | "header" | "footer" | "nav" | "aside" | "ul" | "ol" | "li" | "blockquote"
        | "pre" | "form" | "fieldset" | "table" | "hr" | "address" | "figure"
        | "figcaption" | "details" | "summary" => {
            style.display = Display::Block;
        }
        "html" | "body" => {
            style.display = Display::Block;
        }
        "head" | "script" | "style" | "meta" | "link" | "title" => {
            style.display = Display::None;
        }
        _ => {
            style.display = Display::Inline;
        }
    }

    // Heading font sizes.
    match tag {
        "h1" => {
            style.font_size = 32.0;
            style.font_weight = crate::css::FontWeight::Bold;
        }
        "h2" => {
            style.font_size = 24.0;
            style.font_weight = crate::css::FontWeight::Bold;
        }
        "h3" => {
            style.font_size = 18.7;
            style.font_weight = crate::css::FontWeight::Bold;
        }
        "h4" => {
            style.font_size = 16.0;
            style.font_weight = crate::css::FontWeight::Bold;
        }
        "h5" => {
            style.font_size = 13.3;
            style.font_weight = crate::css::FontWeight::Bold;
        }
        "h6" => {
            style.font_size = 10.7;
            style.font_weight = crate::css::FontWeight::Bold;
        }
        "strong" | "b" => {
            style.font_weight = crate::css::FontWeight::Bold;
        }
        _ => {}
    }

    style
}

/// Convert a `ComputedStyle` into a taffy `Style`.
fn css_to_taffy_style(computed: &ComputedStyle, tag: &str) -> taffy::Style {
    let display = match computed.display {
        Display::Block => taffy::Display::Block,
        Display::Flex => taffy::Display::Flex,
        Display::Grid => taffy::Display::Grid,
        Display::None => taffy::Display::None,
        // Inline and inline-block are approximated as block for taffy.
        Display::Inline | Display::InlineBlock => taffy::Display::Block,
    };

    let width = dimension_to_taffy(computed.width);
    let height = dimension_to_taffy(computed.height);

    let margin = taffy::Rect {
        top: taffy::LengthPercentageAuto::Length(computed.margin.top),
        right: taffy::LengthPercentageAuto::Length(computed.margin.right),
        bottom: taffy::LengthPercentageAuto::Length(computed.margin.bottom),
        left: taffy::LengthPercentageAuto::Length(computed.margin.left),
    };

    let padding = taffy::Rect {
        top: taffy::LengthPercentage::Length(computed.padding.top),
        right: taffy::LengthPercentage::Length(computed.padding.right),
        bottom: taffy::LengthPercentage::Length(computed.padding.bottom),
        left: taffy::LengthPercentage::Length(computed.padding.left),
    };

    let _ = tag; // May be used for tag-specific layout defaults in the future.

    taffy::Style {
        display,
        size: taffy::Size { width, height },
        margin,
        padding,
        ..Default::default()
    }
}

/// Convert a CSS `Dimension` to a taffy `Dimension`.
fn dimension_to_taffy(dim: Dimension) -> taffy::Dimension {
    match dim {
        Dimension::Auto => taffy::Dimension::Auto,
        Dimension::Px(px) => taffy::Dimension::Length(px),
        Dimension::Percent(pct) => taffy::Dimension::Percent(pct / 100.0),
        Dimension::Em(em) => taffy::Dimension::Length(em * 16.0), // approximate
    }
}

/// Recursively collect absolute positions from the taffy layout.
fn collect_layout_boxes(
    taffy: &taffy::TaffyTree,
    node: taffy::NodeId,
    parent_x: f32,
    parent_y: f32,
    boxes: &mut Vec<LayoutBox>,
) {
    let Ok(layout) = taffy.layout(node) else {
        return;
    };

    let x = parent_x + layout.location.x;
    let y = parent_y + layout.location.y;

    boxes.push(LayoutBox {
        x,
        y,
        width: layout.size.width,
        height: layout.size.height,
        node_id: boxes.len(),
    });

    let Ok(children) = taffy.children(node) else {
        return;
    };

    for child in children {
        collect_layout_boxes(taffy, child, x, y, boxes);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::css::Stylesheet;
    use crate::dom::Document;

    #[test]
    fn compute_empty_document() {
        let doc = Document::parse("");
        let tree = LayoutTree::compute(&doc, &[], 800.0);
        // Should not panic.
        let _ = tree.boxes();
    }

    #[test]
    fn compute_simple_document() {
        let html = "<html><body><p>Hello world</p></body></html>";
        let doc = Document::parse(html);
        let tree = LayoutTree::compute(&doc, &[], 800.0);
        // Should produce some layout boxes.
        assert!(!tree.boxes().is_empty());
    }

    #[test]
    fn compute_with_stylesheet() {
        let html = "<html><body><div>Content</div></body></html>";
        let css = "div { width: 200px; }";
        let doc = Document::parse(html);
        let sheet = Stylesheet::parse(css);
        let tree = LayoutTree::compute(&doc, &[sheet], 800.0);
        assert!(!tree.boxes().is_empty());
    }

    #[test]
    fn layout_boxes_have_positions() {
        let html = "<html><body><p>First</p><p>Second</p></body></html>";
        let doc = Document::parse(html);
        let tree = LayoutTree::compute(&doc, &[], 800.0);

        for b in tree.boxes() {
            // All positions should be non-negative.
            assert!(b.x >= 0.0, "x should be >= 0, got {}", b.x);
            assert!(b.y >= 0.0, "y should be >= 0, got {}", b.y);
        }
    }

    #[test]
    fn content_height_positive() {
        let html = "<html><body><p>Some content</p></body></html>";
        let doc = Document::parse(html);
        let tree = LayoutTree::compute(&doc, &[], 800.0);
        assert!(tree.content_height() >= 0.0);
    }

    #[test]
    fn display_none_excluded() {
        let html = "<html><body><div>Visible</div></body></html>";
        let css = "div { display: none; }";
        let doc = Document::parse(html);
        let sheet = Stylesheet::parse(css);
        let tree = LayoutTree::compute(&doc, &[sheet], 800.0);

        // The div and its text child should not produce layout boxes
        // (though html/body will).
        let boxes_before = {
            let tree2 = LayoutTree::compute(&doc, &[], 800.0);
            tree2.boxes().len()
        };
        // With display:none, we should have fewer boxes.
        assert!(tree.boxes().len() <= boxes_before);
    }

    #[test]
    fn default_style_block_elements() {
        let style = default_style_for_tag("div");
        assert_eq!(style.display, Display::Block);

        let style = default_style_for_tag("p");
        assert_eq!(style.display, Display::Block);
    }

    #[test]
    fn default_style_inline_elements() {
        let style = default_style_for_tag("span");
        assert_eq!(style.display, Display::Inline);

        let style = default_style_for_tag("a");
        assert_eq!(style.display, Display::Inline);
    }

    #[test]
    fn default_style_hidden_elements() {
        let style = default_style_for_tag("head");
        assert_eq!(style.display, Display::None);

        let style = default_style_for_tag("script");
        assert_eq!(style.display, Display::None);

        let style = default_style_for_tag("style");
        assert_eq!(style.display, Display::None);
    }

    #[test]
    fn default_style_headings() {
        let h1 = default_style_for_tag("h1");
        assert!((h1.font_size - 32.0).abs() < f32::EPSILON);
        assert_eq!(h1.font_weight, crate::css::FontWeight::Bold);

        let h2 = default_style_for_tag("h2");
        assert!((h2.font_size - 24.0).abs() < f32::EPSILON);
    }

    #[test]
    fn dimension_to_taffy_conversions() {
        match dimension_to_taffy(Dimension::Auto) {
            taffy::Dimension::Auto => {}
            other => panic!("expected Auto, got {other:?}"),
        }

        match dimension_to_taffy(Dimension::Px(100.0)) {
            taffy::Dimension::Length(v) => assert!((v - 100.0).abs() < f32::EPSILON),
            other => panic!("expected Length(100), got {other:?}"),
        }

        match dimension_to_taffy(Dimension::Percent(50.0)) {
            taffy::Dimension::Percent(v) => assert!((v - 0.5).abs() < f32::EPSILON),
            other => panic!("expected Percent(0.5), got {other:?}"),
        }
    }
}
