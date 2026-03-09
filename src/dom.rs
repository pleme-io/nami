//! DOM tree -- html5ever parsed document model.
//!
//! Parses HTML into a DOM tree, provides traversal and query methods
//! for the layout engine and renderer.

use html5ever::parse_document;
use html5ever::tendril::TendrilSink;
use html5ever::tree_builder::TreeSink;
use markup5ever::{namespace_url, Attribute, QualName};
use std::cell::UnsafeCell;
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

/// Information about a link found in the document.
#[derive(Debug, Clone)]
pub struct LinkInfo {
    /// The href URL.
    pub href: String,
    /// The link text content.
    pub text: String,
    /// Link index (0-based order of appearance).
    pub index: usize,
}

/// Information about a form found in the document.
#[derive(Debug, Clone)]
pub struct FormInfo {
    /// Form action URL.
    pub action: String,
    /// Form method (GET, POST).
    pub method: String,
    /// Input fields within the form.
    pub inputs: Vec<InputInfo>,
}

/// Information about a form input field.
#[derive(Debug, Clone)]
pub struct InputInfo {
    /// Input name attribute.
    pub name: String,
    /// Input type (text, password, hidden, etc.).
    pub input_type: String,
    /// Input value.
    pub value: String,
    /// Placeholder text.
    pub placeholder: String,
}

/// Information about an image in the document.
#[derive(Debug, Clone)]
pub struct ImageInfo {
    /// Image source URL.
    pub src: String,
    /// Alt text.
    pub alt: String,
    /// Width attribute if specified.
    pub width: Option<u32>,
    /// Height attribute if specified.
    pub height: Option<u32>,
}

// ---------------------------------------------------------------------------
// html5ever sink -- collects parse events into our Node tree
// ---------------------------------------------------------------------------

/// Handle used by the tree builder to refer to nodes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct NodeHandle(usize);

/// A simple tree-building sink for html5ever.
///
/// Uses `UnsafeCell` for interior mutability because html5ever's `TreeSink`
/// trait methods take `&self` but need to mutate internal state. html5ever
/// guarantees single-threaded sequential access, so this is sound.
struct DomSink {
    inner: UnsafeCell<DomSinkInner>,
}

struct DomSinkInner {
    nodes: Vec<Node>,
    /// Parent index for each node (None for the root document).
    parents: Vec<Option<usize>>,
    /// QualName for each node (needed by html5ever's elem_name).
    qualnames: Vec<QualName>,
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
        let doc_qualname = QualName::new(
            None,
            html5ever::ns!(html),
            html5ever::local_name!("html"),
        );
        Self {
            inner: UnsafeCell::new(DomSinkInner {
                nodes: vec![doc_node],
                parents: vec![None],
                qualnames: vec![doc_qualname],
            }),
        }
    }

    /// Get a mutable reference to the inner data.
    ///
    /// # Safety
    /// Caller must ensure no other references to the inner data exist.
    /// html5ever guarantees single-threaded sequential access to TreeSink.
    #[allow(clippy::mut_from_ref)]
    unsafe fn inner_mut(&self) -> &mut DomSinkInner {
        unsafe { &mut *self.inner.get() }
    }

    /// Get a shared reference to the inner data.
    fn inner_ref(&self) -> &DomSinkInner {
        // SAFETY: We only call this when no mutable references exist.
        unsafe { &*self.inner.get() }
    }

    fn push_node(&self, node: Node, parent: Option<usize>, qualname: QualName) -> usize {
        // SAFETY: html5ever guarantees single-threaded sequential access.
        let inner = unsafe { self.inner_mut() };
        let idx = inner.nodes.len();
        inner.nodes.push(node);
        inner.parents.push(parent);
        inner.qualnames.push(qualname);
        idx
    }

    /// Build the final tree by attaching children to their parents,
    /// bottom-up, and return the document root.
    fn finish(self) -> Node {
        let inner = self.inner.into_inner();
        let len = inner.nodes.len();
        let mut children_map: Vec<Vec<usize>> = vec![Vec::new(); len];

        for i in 1..len {
            if let Some(parent) = inner.parents[i] {
                children_map[parent].push(i);
            }
        }

        let mut taken: Vec<Option<Node>> = inner.nodes.into_iter().map(Some).collect();

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
        let inner = self.inner_ref();
        &inner.qualnames[*target]
    }

    fn create_element(
        &self,
        name: QualName,
        attrs: Vec<Attribute>,
        _flags: html5ever::tree_builder::ElementFlags,
    ) -> usize {
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

        self.push_node(node, None, name)
    }

    fn create_comment(&self, text: html5ever::tendril::StrTendril) -> usize {
        let node = Node {
            kind: NodeKind::Comment(text.to_string()),
        };
        let qn = QualName::new(None, html5ever::ns!(), html5ever::local_name!(""));
        self.push_node(node, None, qn)
    }

    fn create_pi(
        &self,
        _target: html5ever::tendril::StrTendril,
        _data: html5ever::tendril::StrTendril,
    ) -> usize {
        let node = Node {
            kind: NodeKind::Comment(String::new()),
        };
        let qn = QualName::new(None, html5ever::ns!(), html5ever::local_name!(""));
        self.push_node(node, None, qn)
    }

    fn append(&self, parent: &usize, child: html5ever::tree_builder::NodeOrText<usize>) {
        // SAFETY: html5ever guarantees single-threaded sequential access.
        let inner = unsafe { self.inner_mut() };

        match child {
            html5ever::tree_builder::NodeOrText::AppendNode(idx) => {
                inner.parents[idx] = Some(*parent);
            }
            html5ever::tree_builder::NodeOrText::AppendText(text) => {
                let text_str = text.to_string();
                if text_str.is_empty() {
                    return;
                }
                let node = Node {
                    kind: NodeKind::Text(text_str),
                };
                inner.nodes.push(node);
                inner.parents.push(Some(*parent));
                inner.qualnames.push(QualName::new(
                    None,
                    html5ever::ns!(),
                    html5ever::local_name!(""),
                ));
            }
        }
    }

    fn append_based_on_parent_node(
        &self,
        element: &usize,
        prev_element: &usize,
        child: html5ever::tree_builder::NodeOrText<usize>,
    ) {
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
        let parent = self.inner_ref().parents[*sibling].unwrap_or(0);
        self.append(&parent, new_node);
    }

    fn add_attrs_if_missing(&self, target: &usize, attrs: Vec<Attribute>) {
        // SAFETY: html5ever guarantees single-threaded sequential access.
        let inner = unsafe { self.inner_mut() };
        if let NodeKind::Element(ref mut elem) = inner.nodes[*target].kind {
            for attr in attrs {
                elem.attrs
                    .entry(attr.name.local.to_string())
                    .or_insert_with(|| attr.value.to_string());
            }
        }
    }

    fn remove_from_parent(&self, target: &usize) {
        // SAFETY: html5ever guarantees single-threaded sequential access.
        let inner = unsafe { self.inner_mut() };
        inner.parents[*target] = None;
    }

    fn reparent_children(&self, node: &usize, new_parent: &usize) {
        // SAFETY: html5ever guarantees single-threaded sequential access.
        let inner = unsafe { self.inner_mut() };
        let len = inner.parents.len();
        for i in 0..len {
            if inner.parents[i] == Some(*node) {
                inner.parents[i] = Some(*new_parent);
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

    /// Get detailed link information including text and index.
    #[must_use]
    pub fn link_infos(&self) -> Vec<LinkInfo> {
        let mut infos = Vec::new();
        collect_link_infos(&self.root, &mut infos);
        infos
    }

    /// Extract all text content from the document, stripping tags.
    #[must_use]
    pub fn text_content(&self) -> String {
        let mut text = String::new();
        collect_text(&self.root, &mut text);
        let normalised: String = text.split_whitespace().collect::<Vec<_>>().join(" ");
        normalised
    }

    /// Find elements by tag name.
    #[must_use]
    pub fn elements_by_tag(&self, tag: &str) -> Vec<&Element> {
        let mut result = Vec::new();
        find_elements_by_tag(&self.root, tag, &mut result);
        result
    }

    /// Find an element by ID attribute.
    #[must_use]
    pub fn element_by_id(&self, id: &str) -> Option<&Element> {
        find_element_by_id(&self.root, id)
    }

    /// Find elements with a given class name.
    #[must_use]
    pub fn elements_by_class(&self, class: &str) -> Vec<&Element> {
        let mut result = Vec::new();
        find_elements_by_class(&self.root, class, &mut result);
        result
    }

    /// Extract all image information from the document.
    #[must_use]
    pub fn images(&self) -> Vec<ImageInfo> {
        let mut images = Vec::new();
        collect_images(&self.root, &mut images);
        images
    }

    /// Extract form information from the document.
    #[must_use]
    pub fn forms(&self) -> Vec<FormInfo> {
        let mut forms = Vec::new();
        collect_forms(&self.root, &mut forms);
        forms
    }

    /// Get inline stylesheets from `<style>` elements.
    #[must_use]
    pub fn inline_styles(&self) -> Vec<String> {
        let mut styles = Vec::new();
        collect_inline_styles(&self.root, &mut styles);
        styles
    }

    /// Get linked stylesheet URLs from `<link rel="stylesheet">` elements.
    #[must_use]
    pub fn stylesheet_links(&self) -> Vec<String> {
        let mut links = Vec::new();
        collect_stylesheet_links(&self.root, &mut links);
        links
    }

    /// Find the `<body>` element if it exists.
    #[must_use]
    pub fn body(&self) -> Option<&Element> {
        find_element_by_tag(&self.root, "body")
    }

    /// Get meta tag content by name.
    #[must_use]
    pub fn meta_content(&self, name: &str) -> Option<String> {
        let metas = self.elements_by_tag("meta");
        for meta in metas {
            if meta.attrs.get("name").map(String::as_str) == Some(name) {
                return meta.attrs.get("content").cloned();
            }
        }
        None
    }
}

// ---------------------------------------------------------------------------
// Tree traversal helpers
// ---------------------------------------------------------------------------

fn find_title(node: &Node) -> Option<&str> {
    match &node.kind {
        NodeKind::Element(elem) => {
            if elem.tag == "title" {
                for child in &elem.children {
                    if let NodeKind::Text(ref text) = child.kind {
                        return Some(text.as_str());
                    }
                }
                return None;
            }
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

fn collect_link_infos(node: &Node, infos: &mut Vec<LinkInfo>) {
    if let NodeKind::Element(ref elem) = node.kind {
        if elem.tag == "a" {
            if let Some(href) = elem.attrs.get("href") {
                let mut text = String::new();
                collect_text(node, &mut text);
                let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
                infos.push(LinkInfo {
                    href: href.clone(),
                    text,
                    index: infos.len(),
                });
            }
        }
        for child in &elem.children {
            collect_link_infos(child, infos);
        }
    }
}

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

fn find_elements_by_tag<'a>(node: &'a Node, tag: &str, result: &mut Vec<&'a Element>) {
    if let NodeKind::Element(ref elem) = node.kind {
        if elem.tag == tag {
            result.push(elem);
        }
        for child in &elem.children {
            find_elements_by_tag(child, tag, result);
        }
    }
}

fn find_element_by_tag<'a>(node: &'a Node, tag: &str) -> Option<&'a Element> {
    if let NodeKind::Element(ref elem) = node.kind {
        if elem.tag == tag {
            return Some(elem);
        }
        for child in &elem.children {
            if let Some(found) = find_element_by_tag(child, tag) {
                return Some(found);
            }
        }
    }
    None
}

fn find_element_by_id<'a>(node: &'a Node, id: &str) -> Option<&'a Element> {
    if let NodeKind::Element(ref elem) = node.kind {
        if elem.attrs.get("id").map(String::as_str) == Some(id) {
            return Some(elem);
        }
        for child in &elem.children {
            if let Some(found) = find_element_by_id(child, id) {
                return Some(found);
            }
        }
    }
    None
}

fn find_elements_by_class<'a>(node: &'a Node, class: &str, result: &mut Vec<&'a Element>) {
    if let NodeKind::Element(ref elem) = node.kind {
        if let Some(classes) = elem.attrs.get("class") {
            if classes.split_whitespace().any(|c| c == class) {
                result.push(elem);
            }
        }
        for child in &elem.children {
            find_elements_by_class(child, class, result);
        }
    }
}

fn collect_images(node: &Node, images: &mut Vec<ImageInfo>) {
    if let NodeKind::Element(ref elem) = node.kind {
        if elem.tag == "img" {
            let src = elem.attrs.get("src").cloned().unwrap_or_default();
            let alt = elem.attrs.get("alt").cloned().unwrap_or_default();
            let width = elem
                .attrs
                .get("width")
                .and_then(|v| v.parse::<u32>().ok());
            let height = elem
                .attrs
                .get("height")
                .and_then(|v| v.parse::<u32>().ok());
            images.push(ImageInfo {
                src,
                alt,
                width,
                height,
            });
        }
        for child in &elem.children {
            collect_images(child, images);
        }
    }
}

fn collect_forms(node: &Node, forms: &mut Vec<FormInfo>) {
    if let NodeKind::Element(ref elem) = node.kind {
        if elem.tag == "form" {
            let action = elem.attrs.get("action").cloned().unwrap_or_default();
            let method = elem
                .attrs
                .get("method")
                .cloned()
                .unwrap_or_else(|| "GET".to_string())
                .to_uppercase();
            let mut inputs = Vec::new();
            collect_form_inputs(&Node { kind: NodeKind::Element(elem.clone()) }, &mut inputs);
            forms.push(FormInfo {
                action,
                method,
                inputs,
            });
        }
        for child in &elem.children {
            collect_forms(child, forms);
        }
    }
}

fn collect_form_inputs(node: &Node, inputs: &mut Vec<InputInfo>) {
    if let NodeKind::Element(ref elem) = node.kind {
        if elem.tag == "input" || elem.tag == "textarea" {
            let name = elem.attrs.get("name").cloned().unwrap_or_default();
            let input_type = elem
                .attrs
                .get("type")
                .cloned()
                .unwrap_or_else(|| "text".to_string());
            let value = elem.attrs.get("value").cloned().unwrap_or_default();
            let placeholder = elem.attrs.get("placeholder").cloned().unwrap_or_default();
            inputs.push(InputInfo {
                name,
                input_type,
                value,
                placeholder,
            });
        }
        for child in &elem.children {
            collect_form_inputs(child, inputs);
        }
    }
}

fn collect_inline_styles(node: &Node, styles: &mut Vec<String>) {
    if let NodeKind::Element(ref elem) = node.kind {
        if elem.tag == "style" {
            let mut text = String::new();
            for child in &elem.children {
                if let NodeKind::Text(ref t) = child.kind {
                    text.push_str(t);
                }
            }
            if !text.is_empty() {
                styles.push(text);
            }
        }
        for child in &elem.children {
            collect_inline_styles(child, styles);
        }
    }
}

fn collect_stylesheet_links(node: &Node, links: &mut Vec<String>) {
    if let NodeKind::Element(ref elem) = node.kind {
        if elem.tag == "link" {
            if elem.attrs.get("rel").map(String::as_str) == Some("stylesheet") {
                if let Some(href) = elem.attrs.get("href") {
                    links.push(href.clone());
                }
            }
        }
        for child in &elem.children {
            collect_stylesheet_links(child, links);
        }
    }
}

/// Convert a node subtree to plain text with structural formatting.
///
/// Block elements get newlines, headings get emphasis markers, etc.
/// This produces human-readable output suitable for terminal display.
pub fn node_to_text(node: &Node, indent: usize) -> String {
    let mut out = String::new();
    node_to_text_inner(node, indent, &mut out, false);
    out
}

fn node_to_text_inner(node: &Node, indent: usize, out: &mut String, in_pre: bool) {
    match &node.kind {
        NodeKind::Text(text) => {
            if in_pre {
                out.push_str(text);
            } else {
                // Collapse whitespace.
                let collapsed: String = text
                    .split_whitespace()
                    .collect::<Vec<_>>()
                    .join(" ");
                if !collapsed.is_empty() {
                    out.push_str(&collapsed);
                }
            }
        }
        NodeKind::Element(elem) => {
            if elem.tag == "script" || elem.tag == "style" || elem.tag == "head" {
                return;
            }

            let is_block = matches!(
                elem.tag.as_str(),
                "div" | "p" | "h1" | "h2" | "h3" | "h4" | "h5" | "h6"
                    | "section" | "article" | "main" | "header" | "footer"
                    | "nav" | "aside" | "ul" | "ol" | "li" | "blockquote"
                    | "pre" | "form" | "table" | "tr" | "td" | "th"
                    | "dl" | "dt" | "dd" | "figure" | "figcaption"
                    | "details" | "summary" | "hr" | "br"
            );

            let is_pre = elem.tag == "pre" || in_pre;

            // Block-level formatting.
            if is_block && !out.is_empty() && !out.ends_with('\n') {
                out.push('\n');
            }

            // Heading markers.
            match elem.tag.as_str() {
                "h1" => out.push_str("# "),
                "h2" => out.push_str("## "),
                "h3" => out.push_str("### "),
                "h4" => out.push_str("#### "),
                "h5" => out.push_str("##### "),
                "h6" => out.push_str("###### "),
                "li" => {
                    for _ in 0..indent {
                        out.push_str("  ");
                    }
                    out.push_str("- ");
                }
                "hr" => {
                    out.push_str("---\n");
                    return;
                }
                "br" => {
                    out.push('\n');
                    return;
                }
                "blockquote" => {
                    out.push_str("> ");
                }
                _ => {}
            }

            let child_indent = if elem.tag == "ul" || elem.tag == "ol" {
                indent + 1
            } else {
                indent
            };

            for child in &elem.children {
                node_to_text_inner(child, child_indent, out, is_pre);
            }

            // Links: show URL.
            if elem.tag == "a" {
                if let Some(href) = elem.attrs.get("href") {
                    out.push_str(" [");
                    out.push_str(href);
                    out.push(']');
                }
            }

            // Images: show alt text.
            if elem.tag == "img" {
                let alt = elem.attrs.get("alt").map(String::as_str).unwrap_or("[image]");
                out.push_str("[img: ");
                out.push_str(alt);
                out.push(']');
            }

            if is_block && !out.ends_with('\n') {
                out.push('\n');
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
        let html =
            "<html><body><h1>Title</h1><p>Some <strong>bold</strong> text.</p></body></html>";
        let doc = Document::parse(html);
        let text = doc.text_content();
        assert!(text.contains("Title"));
        assert!(text.contains("Some"));
        assert!(text.contains("bold"));
        assert!(text.contains("text."));
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
        let html = "<html><body><p>Unclosed paragraph<div>After div";
        let doc = Document::parse(html);
        let text = doc.text_content();
        assert!(text.contains("Unclosed paragraph"));
        assert!(text.contains("After div"));
    }

    #[test]
    fn parse_empty_string() {
        let doc = Document::parse("");
        assert!(doc.title().is_none());
        assert!(doc.links().is_empty());
    }

    #[test]
    fn element_attributes() {
        let html =
            r#"<html><body><a href="/test" class="link" id="main-link">Click</a></body></html>"#;
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
        assert_eq!(
            anchor.attrs.get("class").map(String::as_str),
            Some("link")
        );
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
        assert!(!text.contains("   "));
        assert!(text.contains("Lots of spaces"));
    }

    #[test]
    fn link_infos_include_text() {
        let html = r#"<html><body>
            <a href="/one">First Link</a>
            <a href="/two">Second Link</a>
        </body></html>"#;
        let doc = Document::parse(html);
        let infos = doc.link_infos();
        assert_eq!(infos.len(), 2);
        assert_eq!(infos[0].href, "/one");
        assert!(infos[0].text.contains("First Link"));
        assert_eq!(infos[0].index, 0);
        assert_eq!(infos[1].index, 1);
    }

    #[test]
    fn find_by_id() {
        let html = r#"<html><body><div id="main">Content</div></body></html>"#;
        let doc = Document::parse(html);
        let elem = doc.element_by_id("main");
        assert!(elem.is_some());
        assert_eq!(elem.unwrap().tag, "div");
    }

    #[test]
    fn find_by_class() {
        let html = r#"<html><body>
            <div class="item active">One</div>
            <div class="item">Two</div>
            <div class="other">Three</div>
        </body></html>"#;
        let doc = Document::parse(html);
        let items = doc.elements_by_class("item");
        assert_eq!(items.len(), 2);
    }

    #[test]
    fn collect_images_info() {
        let html = r#"<html><body>
            <img src="/logo.png" alt="Logo" width="100" height="50">
            <img src="/photo.jpg" alt="Photo">
        </body></html>"#;
        let doc = Document::parse(html);
        let imgs = doc.images();
        assert_eq!(imgs.len(), 2);
        assert_eq!(imgs[0].src, "/logo.png");
        assert_eq!(imgs[0].alt, "Logo");
        assert_eq!(imgs[0].width, Some(100));
        assert_eq!(imgs[1].height, None);
    }

    #[test]
    fn inline_styles_extracted() {
        let html = r#"<html><head><style>body { color: red; }</style></head><body></body></html>"#;
        let doc = Document::parse(html);
        let styles = doc.inline_styles();
        assert_eq!(styles.len(), 1);
        assert!(styles[0].contains("color"));
    }

    #[test]
    fn meta_content_lookup() {
        let html = r#"<html><head><meta name="description" content="A test page"></head><body></body></html>"#;
        let doc = Document::parse(html);
        let desc = doc.meta_content("description");
        assert_eq!(desc.as_deref(), Some("A test page"));
    }

    #[test]
    fn node_to_text_formatting() {
        let html = "<html><body><h1>Title</h1><p>Paragraph</p><ul><li>Item 1</li><li>Item 2</li></ul></body></html>";
        let doc = Document::parse(html);
        if let Some(body) = doc.body() {
            let text = node_to_text(
                &Node {
                    kind: NodeKind::Element(body.clone()),
                },
                0,
            );
            assert!(text.contains("# Title"));
            assert!(text.contains("Paragraph"));
            assert!(text.contains("- Item"));
        }
    }
}
