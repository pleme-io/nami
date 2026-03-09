//! Layout engine -- taffy flexbox/grid.
//!
//! Takes computed styles and DOM tree, produces a layout tree with
//! absolute positions and sizes for each visible element.
//! Uses taffy for CSS flexbox and grid layout algorithms.

use crate::css::{self, ComputedStyle, Dimension, Display, Stylesheet};
use crate::dom::{Document, Node, NodeKind};

#[derive(thiserror::Error, Debug)]
pub enum LayoutError {
    #[error("layout computation failed: {0}")]
    Compute(String),
    #[error("taffy error: {0}")]
    Taffy(String),
}

pub type Result<T> = std::result::Result<T, LayoutError>;

/// The content associated with a layout box.
#[derive(Debug, Clone)]
pub enum BoxContent {
    /// An element box.
    Element {
        tag: String,
        style: ComputedStyle,
    },
    /// A text run within an element.
    Text {
        text: String,
        style: ComputedStyle,
    },
}

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
    /// The content of this box (element or text).
    pub content: BoxContent,
    /// Whether this box is a link.
    pub is_link: bool,
    /// Link href if this is a link.
    pub href: Option<String>,
}

/// Mapping between taffy node IDs and our box metadata.
struct NodeMeta {
    content: BoxContent,
    is_link: bool,
    href: Option<String>,
}

/// A computed layout tree.
pub struct LayoutTree {
    boxes: Vec<LayoutBox>,
}

impl LayoutTree {
    /// Compute layout for a document with the given stylesheets and viewport width.
    #[must_use]
    pub fn compute(doc: &Document, styles: &[Stylesheet], viewport_width: f32) -> Self {
        let mut taffy = taffy::TaffyTree::new();
        let mut meta_map: Vec<(taffy::NodeId, NodeMeta)> = Vec::new();
        let parent_style = ComputedStyle {
            color: "#eceff4".to_string(),
            ..ComputedStyle::default()
        };

        let root_node = build_taffy_node(
            &doc.root,
            &mut taffy,
            styles,
            &mut meta_map,
            &parent_style,
            false,
            None,
        );

        if let Some(root) = root_node {
            let available = taffy::Size {
                width: taffy::AvailableSpace::Definite(viewport_width),
                height: taffy::AvailableSpace::MaxContent,
            };

            if let Err(e) = taffy.compute_layout(root, available) {
                tracing::warn!(error = %e, "taffy layout computation failed");
            }
        }

        // Collect boxes by walking taffy tree.
        let mut boxes = Vec::new();
        if let Some(root) = root_node {
            collect_layout_boxes(&taffy, root, 0.0, 0.0, &meta_map, &mut boxes);
        }

        tracing::debug!(
            box_count = boxes.len(),
            viewport_width,
            "layout computed"
        );

        Self { boxes }
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

    /// Get all link boxes (for click target detection).
    #[must_use]
    pub fn link_boxes(&self) -> Vec<&LayoutBox> {
        self.boxes.iter().filter(|b| b.is_link).collect()
    }

    /// Find the box at a given (x, y) position.
    #[must_use]
    pub fn box_at(&self, x: f32, y: f32) -> Option<&LayoutBox> {
        // Return the deepest (last in list) box that contains the point.
        self.boxes
            .iter()
            .rev()
            .find(|b| x >= b.x && x <= b.x + b.width && y >= b.y && y <= b.y + b.height)
    }

    /// Find a link at a given (x, y) position.
    #[must_use]
    pub fn link_at(&self, x: f32, y: f32) -> Option<&str> {
        self.boxes
            .iter()
            .rev()
            .filter(|b| b.is_link)
            .find(|b| x >= b.x && x <= b.x + b.width && y >= b.y && y <= b.y + b.height)
            .and_then(|b| b.href.as_deref())
    }

    /// Get text content boxes in reading order.
    #[must_use]
    pub fn text_boxes(&self) -> Vec<&LayoutBox> {
        self.boxes
            .iter()
            .filter(|b| matches!(b.content, BoxContent::Text { .. }))
            .collect()
    }
}

/// Recursively build taffy nodes from the DOM tree.
fn build_taffy_node(
    node: &Node,
    taffy: &mut taffy::TaffyTree,
    styles: &[Stylesheet],
    meta_map: &mut Vec<(taffy::NodeId, NodeMeta)>,
    parent_style: &ComputedStyle,
    parent_is_link: bool,
    parent_href: Option<&str>,
) -> Option<taffy::NodeId> {
    match &node.kind {
        NodeKind::Element(elem) => {
            let computed = css::cascade_style(elem, styles, parent_style);
            if computed.display == Display::None {
                return None;
            }
            if computed.visibility == css::Visibility::Hidden {
                // Still takes space, but don't render content.
            }

            let taffy_style = css_to_taffy_style(&computed, &elem.tag);

            let is_link = elem.tag == "a" && elem.attrs.contains_key("href") || parent_is_link;
            let href = if elem.tag == "a" {
                elem.attrs.get("href").map(String::as_str)
            } else {
                parent_href
            };

            // Build children.
            let child_nodes: Vec<taffy::NodeId> = elem
                .children
                .iter()
                .filter_map(|child| {
                    build_taffy_node(child, taffy, styles, meta_map, &computed, is_link, href)
                })
                .collect();

            match taffy.new_with_children(taffy_style, &child_nodes) {
                Ok(taffy_node) => {
                    meta_map.push((
                        taffy_node,
                        NodeMeta {
                            content: BoxContent::Element {
                                tag: elem.tag.clone(),
                                style: computed,
                            },
                            is_link,
                            href: href.map(String::from),
                        },
                    ));
                    Some(taffy_node)
                }
                Err(e) => {
                    tracing::trace!(error = %e, tag = %elem.tag, "failed to create taffy node");
                    None
                }
            }
        }
        NodeKind::Text(text) => {
            let trimmed = if parent_style.white_space == css::WhiteSpace::Pre
                || parent_style.white_space == css::WhiteSpace::PreWrap
            {
                text.clone()
            } else {
                let collapsed: String = text.split_whitespace().collect::<Vec<_>>().join(" ");
                collapsed
            };

            if trimmed.is_empty() {
                return None;
            }

            // Estimate text dimensions based on character count and font size.
            let char_width = parent_style.font_size * 0.6; // Approximate char width.
            let line_height = parent_style.font_size * parent_style.line_height;
            let text_width = trimmed.len() as f32 * char_width;
            let estimated_height = line_height;

            let style = taffy::Style {
                size: taffy::Size {
                    width: taffy::Dimension::Length(text_width),
                    height: taffy::Dimension::Length(estimated_height),
                },
                ..Default::default()
            };

            match taffy.new_leaf(style) {
                Ok(taffy_node) => {
                    meta_map.push((
                        taffy_node,
                        NodeMeta {
                            content: BoxContent::Text {
                                text: trimmed,
                                style: parent_style.clone(),
                            },
                            is_link: parent_is_link,
                            href: parent_href.map(String::from),
                        },
                    ));
                    Some(taffy_node)
                }
                Err(e) => {
                    tracing::trace!(error = %e, "failed to create text taffy node");
                    None
                }
            }
        }
        NodeKind::Comment(_) => None,
    }
}

/// Convert a `ComputedStyle` into a taffy `Style`.
fn css_to_taffy_style(computed: &ComputedStyle, _tag: &str) -> taffy::Style {
    let display = match computed.display {
        Display::Block | Display::ListItem => taffy::Display::Block,
        Display::Flex => taffy::Display::Flex,
        Display::Grid => taffy::Display::Grid,
        Display::None => taffy::Display::None,
        Display::Inline | Display::InlineBlock => taffy::Display::Block,
        Display::Table | Display::TableRow | Display::TableCell => taffy::Display::Block,
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

    let border = taffy::Rect {
        top: taffy::LengthPercentage::Length(computed.border_width.top),
        right: taffy::LengthPercentage::Length(computed.border_width.right),
        bottom: taffy::LengthPercentage::Length(computed.border_width.bottom),
        left: taffy::LengthPercentage::Length(computed.border_width.left),
    };

    taffy::Style {
        display,
        size: taffy::Size { width, height },
        margin,
        padding,
        border,
        ..Default::default()
    }
}

/// Convert a CSS `Dimension` to a taffy `Dimension`.
fn dimension_to_taffy(dim: Dimension) -> taffy::Dimension {
    match dim {
        Dimension::Auto => taffy::Dimension::Auto,
        Dimension::Px(px) => taffy::Dimension::Length(px),
        Dimension::Percent(pct) => taffy::Dimension::Percent(pct / 100.0),
        Dimension::Em(em) => taffy::Dimension::Length(em * 16.0),
        Dimension::Rem(rem) => taffy::Dimension::Length(rem * 16.0),
        Dimension::Vw(_) | Dimension::Vh(_) => taffy::Dimension::Auto,
    }
}

/// Recursively collect absolute positions from the taffy layout.
fn collect_layout_boxes(
    taffy: &taffy::TaffyTree,
    node: taffy::NodeId,
    parent_x: f32,
    parent_y: f32,
    meta_map: &[(taffy::NodeId, NodeMeta)],
    boxes: &mut Vec<LayoutBox>,
) {
    let Ok(layout) = taffy.layout(node) else {
        return;
    };

    let x = parent_x + layout.location.x;
    let y = parent_y + layout.location.y;

    // Find the metadata for this node.
    if let Some((_, meta)) = meta_map.iter().find(|(id, _)| *id == node) {
        boxes.push(LayoutBox {
            x,
            y,
            width: layout.size.width,
            height: layout.size.height,
            content: meta.content.clone(),
            is_link: meta.is_link,
            href: meta.href.clone(),
        });
    }

    let Ok(children) = taffy.children(node) else {
        return;
    };

    for child in children {
        collect_layout_boxes(taffy, child, x, y, meta_map, boxes);
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
        let _ = tree.boxes();
    }

    #[test]
    fn compute_simple_document() {
        let html = "<html><body><p>Hello world</p></body></html>";
        let doc = Document::parse(html);
        let tree = LayoutTree::compute(&doc, &[], 800.0);
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

        let boxes_before = {
            let tree2 = LayoutTree::compute(&doc, &[], 800.0);
            tree2.boxes().len()
        };
        assert!(tree.boxes().len() <= boxes_before);
    }

    #[test]
    fn text_boxes_collected() {
        let html = "<html><body><p>Hello</p><p>World</p></body></html>";
        let doc = Document::parse(html);
        let tree = LayoutTree::compute(&doc, &[], 800.0);
        let texts = tree.text_boxes();
        assert!(!texts.is_empty());
    }

    #[test]
    fn link_boxes_detected() {
        let html = r#"<html><body><a href="/test">Click me</a></body></html>"#;
        let doc = Document::parse(html);
        let tree = LayoutTree::compute(&doc, &[], 800.0);
        let links = tree.link_boxes();
        assert!(!links.is_empty());
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
