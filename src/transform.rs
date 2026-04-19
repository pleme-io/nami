//! Lisp-authored DOM transforms applied during page load.
//!
//! Reuses [`nami_core::transform::DomTransformSpec`] + [`DomAction`] as
//! the spec surface so transforms authored once work against both the
//! nami-core `Document` (e.g. in unit tests) and nami's own DOM. The
//! execution engine is ported to nami's `NodeKind`/`Element` shape
//! because the two DOM types haven't converged yet — the dedup arc is
//! queued separately.
//!
//! Load user transforms at startup from `~/.config/nami/transforms.lisp`,
//! apply them post-parse, pre-layout. See `TransformSet::load`.

use crate::dom::{Document, Element, Node, NodeKind};
use nami_core::transform::{DomAction, DomTransformSpec, TransformHit, TransformReport};
use std::path::Path;

/// A compiled bundle of Lisp-authored transforms ready to apply.
#[derive(Debug, Clone, Default)]
pub struct TransformSet {
    pub specs: Vec<DomTransformSpec>,
}

impl TransformSet {
    /// Compile Lisp source directly.
    pub fn from_str(src: &str) -> Result<Self, String> {
        let specs = nami_core::transform::compile(src)?;
        Ok(Self { specs })
    }

    /// Load transforms from a file. Absent file = empty set, not an error.
    pub fn load(path: &Path) -> Result<Self, String> {
        if !path.exists() {
            return Ok(Self::default());
        }
        let src = std::fs::read_to_string(path).map_err(|e| format!("read {path:?}: {e}"))?;
        Self::from_str(&src)
    }

    /// Apply every spec to a document, in author order.
    pub fn apply(&self, doc: &mut Document) -> TransformReport {
        apply(doc, &self.specs)
    }

    pub fn len(&self) -> usize {
        self.specs.len()
    }

    pub fn is_empty(&self) -> bool {
        self.specs.is_empty()
    }
}

/// Apply a sequence of transforms to a document, in order.
pub fn apply(doc: &mut Document, transforms: &[DomTransformSpec]) -> TransformReport {
    let mut report = TransformReport::default();
    for spec in transforms {
        apply_one(&mut doc.root, spec, &mut report);
    }
    report
}

fn apply_one(node: &mut Node, spec: &DomTransformSpec, report: &mut TransformReport) {
    // Depth-first: descendants first so structural mutations at a parent
    // level see the post-transform subtree.
    if let NodeKind::Element(el) = &mut node.kind {
        for child in &mut el.children {
            apply_one(child, spec, report);
        }
    }

    match spec.action {
        DomAction::Remove => {
            if let NodeKind::Element(el) = &mut node.kind {
                let before = el.children.len();
                el.children.retain(|c| !matches_element(c, &spec.selector));
                let removed = before - el.children.len();
                for _ in 0..removed {
                    report.applied.push(TransformHit {
                        transform: spec.name.clone(),
                        action: spec.action,
                        tag: selector_tag(&spec.selector),
                    });
                }
            }
        }
        DomAction::Unwrap => {
            if let NodeKind::Element(parent) = &mut node.kind {
                let mut new_children: Vec<Node> = Vec::with_capacity(parent.children.len());
                for child in std::mem::take(&mut parent.children) {
                    if matches_element(&child, &spec.selector) {
                        let (child_tag, child_children) = match child.kind {
                            NodeKind::Element(e) => (e.tag, e.children),
                            _ => (String::new(), Vec::new()),
                        };
                        report.applied.push(TransformHit {
                            transform: spec.name.clone(),
                            action: spec.action,
                            tag: child_tag,
                        });
                        new_children.extend(child_children);
                    } else {
                        new_children.push(child);
                    }
                }
                parent.children = new_children;
            }
        }
        DomAction::AddClass
        | DomAction::RemoveClass
        | DomAction::SetAttr
        | DomAction::RemoveAttr
        | DomAction::SetText => {
            if let NodeKind::Element(parent) = &mut node.kind {
                for child in &mut parent.children {
                    if matches_element(child, &spec.selector) {
                        if let Some(hit) = apply_in_place(child, spec) {
                            report.applied.push(hit);
                        }
                    }
                }
            }
        }
    }
}

fn apply_in_place(node: &mut Node, spec: &DomTransformSpec) -> Option<TransformHit> {
    let arg = spec.arg.as_deref();
    let NodeKind::Element(el) = &mut node.kind else {
        return None;
    };
    let tag_for_hit = el.tag.clone();

    match spec.action {
        DomAction::AddClass => add_class(el, arg?),
        DomAction::RemoveClass => remove_class(el, arg?),
        DomAction::SetAttr => {
            let (name, value) = arg?.split_once('=')?;
            el.attrs.insert(name.to_owned(), value.to_owned());
        }
        DomAction::RemoveAttr => {
            el.attrs.remove(arg?);
        }
        DomAction::SetText => {
            el.children = vec![Node {
                kind: NodeKind::Text(arg?.to_owned()),
            }];
        }
        _ => return None,
    }

    Some(TransformHit {
        transform: spec.name.clone(),
        action: spec.action,
        tag: tag_for_hit,
    })
}

fn add_class(el: &mut Element, class: &str) {
    let current = el.attrs.get("class").cloned().unwrap_or_default();
    let already = current.split_whitespace().any(|c| c == class);
    if already {
        return;
    }
    let new_val = if current.is_empty() {
        class.to_owned()
    } else {
        format!("{current} {class}")
    };
    el.attrs.insert("class".to_owned(), new_val);
}

fn remove_class(el: &mut Element, class: &str) {
    let Some(current) = el.attrs.get("class") else {
        return;
    };
    let filtered: Vec<&str> = current.split_whitespace().filter(|c| *c != class).collect();
    if filtered.is_empty() {
        el.attrs.remove("class");
    } else {
        el.attrs.insert("class".to_owned(), filtered.join(" "));
    }
}

fn matches_element(node: &Node, selector: &str) -> bool {
    let NodeKind::Element(el) = &node.kind else {
        return false;
    };
    let sel = selector.trim();
    if let Some(class) = sel.strip_prefix('.') {
        el.attrs
            .get("class")
            .is_some_and(|classes| classes.split_whitespace().any(|c| c == class))
    } else if let Some(id) = sel.strip_prefix('#') {
        el.attrs.get("id").is_some_and(|v| v == id)
    } else {
        el.tag.eq_ignore_ascii_case(sel)
    }
}

fn selector_tag(selector: &str) -> String {
    selector.trim_start_matches(['.', '#']).to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dom::Document;

    fn count_tag(doc: &Document, tag: &str) -> usize {
        fn walk(node: &Node, tag: &str, n: &mut usize) {
            if let NodeKind::Element(el) = &node.kind {
                if el.tag == tag {
                    *n += 1;
                }
                for c in &el.children {
                    walk(c, tag, n);
                }
            }
        }
        let mut n = 0;
        walk(&doc.root, tag, &mut n);
        n
    }

    fn find_tag<'a>(doc: &'a Document, tag: &str) -> Option<&'a Element> {
        fn walk<'a>(node: &'a Node, tag: &str) -> Option<&'a Element> {
            if let NodeKind::Element(el) = &node.kind {
                if el.tag == tag {
                    return Some(el);
                }
                for c in &el.children {
                    if let Some(found) = walk(c, tag) {
                        return Some(found);
                    }
                }
            }
            None
        }
        walk(&doc.root, tag)
    }

    #[test]
    fn remove_strips_matching_elements() {
        let mut doc = Document::parse(
            r#"<html><body><div class="ad">a</div><p>ok</p><div class="ad">b</div></body></html>"#,
        );
        let set = TransformSet::from_str(
            r#"(defdom-transform :name "hide-ads" :selector ".ad" :action remove)"#,
        )
        .unwrap();
        let report = set.apply(&mut doc);
        assert_eq!(count_tag(&doc, "div"), 0);
        assert_eq!(count_tag(&doc, "p"), 1);
        assert_eq!(report.applied.len(), 2);
    }

    #[test]
    fn add_class_is_idempotent() {
        let mut doc = Document::parse(r#"<html><body><img src="x"></body></html>"#);
        let set = TransformSet::from_str(
            r#"(defdom-transform :name "flag" :selector "img" :action add-class :arg "needs-alt")"#,
        )
        .unwrap();
        set.apply(&mut doc);
        set.apply(&mut doc);
        let img = find_tag(&doc, "img").unwrap();
        assert_eq!(img.attrs.get("class").unwrap(), "needs-alt");
    }

    #[test]
    fn set_attr_creates_or_updates() {
        let mut doc = Document::parse(r#"<html><body><a href="old">x</a></body></html>"#);
        let set = TransformSet::from_str(
            r#"(defdom-transform :name "rw" :selector "a" :action set-attr :arg "href=https://new")"#,
        )
        .unwrap();
        set.apply(&mut doc);
        assert_eq!(
            find_tag(&doc, "a").unwrap().attrs.get("href").unwrap(),
            "https://new"
        );
    }

    #[test]
    fn unwrap_replaces_with_children() {
        let mut doc =
            Document::parse(r#"<html><body><div class="wrap"><p>inner</p></div></body></html>"#);
        let set = TransformSet::from_str(
            r#"(defdom-transform :name "unwrap" :selector ".wrap" :action unwrap)"#,
        )
        .unwrap();
        set.apply(&mut doc);
        assert_eq!(count_tag(&doc, "div"), 0);
        assert_eq!(count_tag(&doc, "p"), 1);
    }

    #[test]
    fn absent_transforms_file_yields_empty_set() {
        let set = TransformSet::load(Path::new("/nonexistent/path/transforms.lisp")).unwrap();
        assert!(set.is_empty());
    }
}
