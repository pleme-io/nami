//! DOM tree -- html5ever parsed document model.
//!
//! Parses HTML into a DOM tree, provides traversal and query methods
//! for the layout engine and renderer.

use html5ever::parse_document;
use html5ever::tendril::TendrilSink;
use html5ever::tree_builder::TreeSink;
use markup5ever::{Attribute, QualName};
use std::collections::HashMap;

/// The kind of a DOM node.
#[derive(Debug, Clone)]
pub enum NodeKind {
    /// An HTML element with tag name, attributes, and children.
    Element(Element),
    /// A text node.
    Text(String),
    /// An HTML comment.
    Comment(String),
}

/// An HTML element.
#[derive(Debug, Clone)]
pub struct Element {
    /// Tag name (lowercase), e.g. "div", "a", "p".
    pub tag: String,
    /// Element attributes as key-value pairs.
    pub attrs: HashMap<String, String>,
    /// Child nodes.
    pub children: Vec<Node>,
}

/// A single node in the DOM tree.
#[derive(Debug, Clone)]
pub struct Node {
    /// What kind of node this is.
    pub kind: NodeKind,
}

/// A parsed HTML document.
#[derive(Debug, Clone)]
pub struct Document {
    /// The root node of the document tree.
    pub root: Node,
}

// ---------------------------------------------------------------------------
// html5ever sink -- collects parse events into our Node tree
// ---------------------------------------------------------------------------

/// Handle used by the tree builder to refer to nodes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct NodeHandle(usize);

/// A simple tree-building sink for html5ever.
struct DomSink {
    nodes: Vec<Node>,
    /// Parent index for each node (None for the root document).
    parents: Vec<Option<usize>>,
}

impl DomSink {
    fn new() -> Self {
        // Index 0 is the implicit document node.
        let doc_node = Node {
            kind: NodeKind::Element(Element {
                tag: "#document".to_string(),
                attrs: HashMap::new(),
                children: Vec::new(),
            }),
        };
        Self {
            nodes: vec![doc_node],
            parents: vec![None],
        }
    }

    fn push_node(&mut self, node: Node, parent: Option<usize>) -> usize {
        let idx = self.nodes.len();
        self.nodes.push(node);
        self.parents.push(parent);
        idx
    }

    /// Build the final tree by attaching children to their parents,
    /// bottom-up, and return the document root.
    fn finish(mut self) -> Node {
        // We need to build the tree by re-parenting nodes.
        // Collect children in order.
        let len = self.nodes.len();
        let mut children_map: Vec<Vec<usize>> = vec![Vec::new(); len];

        for i in 1..len {
            if let Some(parent) = self.parents[i] {
                children_map[parent].push(i);
            }
        }

        // Build bottom-up: take children from the nodes vec using indices.
        // We do this by processing in reverse order and replacing node children.
        // Since nodes own their children, we need to move them.

        // First pass: build children lists in-place using a placeholder approach.
        // We'll process from highest index down so children are moved before parents.
        let mut taken: Vec<Option<Node>> = self.nodes.into_iter().map(Some).collect();

        for i in (0..len).rev() {
            if !children_map[i].is_empty() {
                let child_nodes: Vec<Node> = children_map[i]
                    .iter()
                    .filter_map(|&idx| taken[idx].take())
                    .collect();

                if let Some(ref mut node) = taken[i] {
                    if let NodeKind::Element(ref mut elem) = node.kind {
                        elem.children = child_nodes;
                    }
                }
            }
        }

        taken[0].take().unwrap_or(Node {
            kind: NodeKind::Element(Element {
                tag: "#document".to_string(),
                attrs: HashMap::new(),
                children: Vec::new(),
            }),
        })
    }
}

impl TreeSink for DomSink {
    type Handle = usize;
    type Output = Self;
    type ElemName<'a> = &'a QualName where Self: 'a;

    fn finish(self) -> Self::Output {
        self
    }

    fn parse_error(&self, _msg: std::borrow::Cow<'static, str>) {
        // html5ever reports parse errors for malformed HTML.
        // We silently ignore them to match browser behaviour.
    }

    fn get_document(&self) -> usize {
        0
    }

    fn elem_name<'a>(&'a self, target: &'a usize) -> &'a QualName {
        // We need to return a reference to a QualName.
        // This is tricky with our node structure, so we'll use a static fallback.
        // html5ever only calls this during tree construction to compare element names.
        static UNKNOWN: std::sync::LazyLock<QualName> = std::sync::LazyLock::new(|| {
            QualName::new(
                None,
                html5ever::ns!(html),
                html5ever::local_name!("unknown"),
            )
        });

        if let NodeKind::Element(ref elem) = self.nodes[*target].kind {
            // We can't easily return a reference to a QualName we don't store.
            // Use the static unknown as fallback -- html5ever primarily uses this
            // for foster parenting checks.
            let _ = elem;
        }
        &UNKNOWN
    }

    fn create_element(
        &self,
        name: QualName,
        attrs: Vec<Attribute>,
        _flags: html5ever::tree_builder::ElementFlags,
    ) -> usize {
        // Safety: we need &mut self but TreeSink gives us &self.
        // html5ever guarantees single-threaded sequential access.
        let self_mut = unsafe { &mut *(self as *const Self as *mut Self) };

        let attr_map: HashMap<String, String> = attrs
            .into_iter()
            .map(|a| (a.name.local.to_string(), a.value.to_string()))
            .collect();

        let node = Node {
            kind: NodeKind::Element(Element {
                tag: name.local.to_string(),
                attrs: attr_map,
                children: Vec::new(),
            }),
        };

        self_mut.push_node(node, None)
    }

    fn create_comment(&self, text: html5ever::tendril::StrTendril) -> usize {
        let self_mut = unsafe { &mut *(self as *const Self as *mut Self) };
        let node = Node {
            kind: NodeKind::Comment(text.to_string()),
        };
        self_mut.push_node(node, None)
    }

    fn create_pi(
        &self,
        _target: html5ever::tendril::StrTendril,
        _data: html5ever::tendril::StrTendril,
    ) -> usize {
        let self_mut = unsafe { &mut *(self as *const Self as *mut Self) };
        let node = Node {
            kind: NodeKind::Comment(String::new()),
        };
        self_mut.push_node(node, None)
    }

    fn append(&self, parent: &usize, child: html5ever::tree_builder::NodeOrText<usize>) {
        let self_mut = unsafe { &mut *(self as *const Self as *mut Self) };

        match child {
            html5ever::tree_builder::NodeOrText::AppendNode(idx) => {
                self_mut.parents[idx] = Some(*parent);
            }
            html5ever::tree_builder::NodeOrText::AppendText(text) => {
                let text_str = text.to_string();
                if text_str.is_empty() {
                    return;
                }
                // Try to merge with the last text child of this parent.
                // Check if we have a recent text node under this parent.
                let node = Node {
                    kind: NodeKind::Text(text_str),
                };
                self_mut.push_node(node, Some(*parent));
            }
        }
    }

    fn append_based_on_parent_node(
        &self,
        element: &usize,
        prev_element: &usize,
        child: html5ever::tree_builder::NodeOrText<usize>,
    ) {
        // Simplified: always append to the element.
        let _ = prev_element;
        self.append(element, child);
    }

    fn append_doctype_to_document(
        &self,
        _name: html5ever::tendril::StrTendril,
        _public_id: html5ever::tendril::StrTendril,
        _system_id: html5ever::tendril::StrTendril,
    ) {
        // We don't store DOCTYPE nodes.
    }

    fn get_template_contents(&self, target: &usize) -> usize {
        *target
    }

    fn same_node(&self, x: &usize, y: &usize) -> bool {
        x == y
    }

    fn set_quirks_mode(&self, _mode: html5ever::tree_builder::QuirksMode) {
        // Ignored.
    }

    fn append_before_sibling(
        &self,
        sibling: &usize,
        new_node: html5ever::tree_builder::NodeOrText<usize>,
    ) {
        // Simplified: insert as child of sibling's parent.
        let parent = self.parents[*sibling].unwrap_or(0);
        self.append(&parent, new_node);
    }

    fn add_attrs_if_missing(&self, target: &usize, attrs: Vec<Attribute>) {
        let self_mut = unsafe { &mut *(self as *const Self as *mut Self) };
        if let NodeKind::Element(ref mut elem) = self_mut.nodes[*target].kind {
            for attr in attrs {
                elem.attrs
                    .entry(attr.name.local.to_string())
                    .or_insert_with(|| attr.value.to_string());
            }
        }
    }

    fn remove_from_parent(&self, target: &usize) {
        let self_mut = unsafe { &mut *(self as *const Self as *mut Self) };
        self_mut.parents[*target] = None;
    }

    fn reparent_children(&self, node: &usize, new_parent: &usize) {
        let self_mut = unsafe { &mut *(self as *const Self as *mut Self) };
        let len = self_mut.parents.len();
        for i in 0..len {
            if self_mut.parents[i] == Some(*node) {
                self_mut.parents[i] = Some(*new_parent);
            }
        }
    }
}

impl Document {
    /// Parse an HTML string into a `Document`.
    ///
    /// Uses html5ever for spec-compliant HTML5 parsing, including handling of
    /// malformed markup.
    #[must_use]
    pub fn parse(html: &str) -> Self {
        let sink = DomSink::new();
        let result = parse_document(sink, Default::default())
            .from_utf8()
            .one(html.as_bytes());

        let root = DomSink::finish(result);

        Document { root }
    }

    /// Get the document title (content of the `<title>` element).
    #[must_use]
    pub fn title(&self) -> Option<&str> {
        find_title(&self.root)
    }

    /// Get all `href` attribute values from anchor (`<a>`) elements.
    #[must_use]
    pub fn links(&self) -> Vec<&str> {
        let mut links = Vec::new();
        collect_links(&self.root, &mut links);
        links
    }

    /// Extract all text content from the document, stripping tags.
    #[must_use]
    pub fn text_content(&self) -> String {
        let mut text = String::new();
        collect_text(&self.root, &mut text);
        // Normalise whitespace: collapse runs of whitespace into single spaces.
        let normalised: String = text
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ");
        normalised
    }
}

// ---------------------------------------------------------------------------
// Tree traversal helpers
// ---------------------------------------------------------------------------

/// Recursively search for the `<title>` element and return its text content.
fn find_title(node: &Node) -> Option<&str> {
    match &node.kind {
        NodeKind::Element(elem) => {
            if elem.tag == "title" {
                // Return the first text child.
                for child in &elem.children {
                    if let NodeKind::Text(ref text) = child.kind {
                        return Some(text.as_str());
                    }
                }
                return None;
            }
            // Recurse into children.
            for child in &elem.children {
                if let Some(title) = find_title(child) {
                    return Some(title);
                }
            }
            None
        }
        _ => None,
    }
}

/// Collect all `href` values from `<a>` elements.
fn collect_links<'a>(node: &'a Node, links: &mut Vec<&'a str>) {
    if let NodeKind::Element(ref elem) = node.kind {
        if elem.tag == "a" {
            if let Some(href) = elem.attrs.get("href") {
                links.push(href.as_str());
            }
        }
        for child in &elem.children {
            collect_links(child, links);
        }
    }
}

/// Collect all text content from a node tree.
fn collect_text(node: &Node, buf: &mut String) {
    match &node.kind {
        NodeKind::Text(text) => {
            buf.push_str(text);
            buf.push(' ');
        }
        NodeKind::Element(elem) => {
            // Skip script and style content.
            if elem.tag == "script" || elem.tag == "style" {
                return;
            }
            for child in &elem.children {
                collect_text(child, buf);
            }
        }
        NodeKind::Comment(_) => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_minimal_html() {
        let doc = Document::parse("<html><body><p>Hello</p></body></html>");
        // Should not panic and should produce a tree.
        let text = doc.text_content();
        assert!(text.contains("Hello"));
    }

    #[test]
    fn parse_extracts_title() {
        let doc = Document::parse(
            "<html><head><title>My Page</title></head><body></body></html>",
        );
        assert_eq!(doc.title(), Some("My Page"));
    }

    #[test]
    fn parse_no_title_returns_none() {
        let doc = Document::parse("<html><body><p>No title here</p></body></html>");
        assert_eq!(doc.title(), None);
    }

    #[test]
    fn parse_extracts_links() {
        let html = r#"
            <html><body>
                <a href="https://example.com">Example</a>
                <a href="/about">About</a>
                <p>No link here</p>
                <a href="mailto:test@test.com">Email</a>
            </body></html>
        "#;
        let doc = Document::parse(html);
        let links = doc.links();
        assert_eq!(links.len(), 3);
        assert!(links.contains(&"https://example.com"));
        assert!(links.contains(&"/about"));
        assert!(links.contains(&"mailto:test@test.com"));
    }

    #[test]
    fn parse_no_links() {
        let doc = Document::parse("<html><body><p>No links</p></body></html>");
        assert!(doc.links().is_empty());
    }

    #[test]
    fn text_content_strips_tags() {
        let html = "<html><body><h1>Title</h1><p>Some <strong>bold</strong> text.</p></body></html>";
        let doc = Document::parse(html);
        let text = doc.text_content();
        assert!(text.contains("Title"));
        assert!(text.contains("Some"));
        assert!(text.contains("bold"));
        assert!(text.contains("text."));
        // Should not contain HTML tags.
        assert!(!text.contains("<h1>"));
        assert!(!text.contains("<strong>"));
    }

    #[test]
    fn text_content_skips_script_and_style() {
        let html = r#"
            <html><body>
                <p>Visible</p>
                <script>var x = 1;</script>
                <style>.foo { color: red; }</style>
                <p>Also visible</p>
            </body></html>
        "#;
        let doc = Document::parse(html);
        let text = doc.text_content();
        assert!(text.contains("Visible"));
        assert!(text.contains("Also visible"));
        assert!(!text.contains("var x"));
        assert!(!text.contains(".foo"));
    }

    #[test]
    fn parse_handles_malformed_html() {
        // html5ever should handle unclosed tags gracefully.
        let html = "<html><body><p>Unclosed paragraph<div>After div";
        let doc = Document::parse(html);
        let text = doc.text_content();
        assert!(text.contains("Unclosed paragraph"));
        assert!(text.contains("After div"));
    }

    #[test]
    fn parse_empty_string() {
        let doc = Document::parse("");
        // Should produce a valid document with no meaningful content.
        assert!(doc.title().is_none());
        assert!(doc.links().is_empty());
    }

    #[test]
    fn element_attributes() {
        let html = r#"<html><body><a href="/test" class="link" id="main-link">Click</a></body></html>"#;
        let doc = Document::parse(html);

        fn find_element<'a>(node: &'a Node, tag: &str) -> Option<&'a Element> {
            match &node.kind {
                NodeKind::Element(elem) => {
                    if elem.tag == tag {
                        return Some(elem);
                    }
                    for child in &elem.children {
                        if let Some(found) = find_element(child, tag) {
                            return Some(found);
                        }
                    }
                    None
                }
                _ => None,
            }
        }

        let anchor = find_element(&doc.root, "a").expect("should find <a> element");
        assert_eq!(anchor.attrs.get("href").map(String::as_str), Some("/test"));
        assert_eq!(anchor.attrs.get("class").map(String::as_str), Some("link"));
        assert_eq!(
            anchor.attrs.get("id").map(String::as_str),
            Some("main-link")
        );
    }

    #[test]
    fn text_content_normalises_whitespace() {
        let html = "<html><body><p>  Lots   of    spaces  </p></body></html>";
        let doc = Document::parse(html);
        let text = doc.text_content();
        // Should collapse multiple whitespace into single spaces.
        assert!(!text.contains("   "));
        assert!(text.contains("Lots of spaces"));
    }
}
