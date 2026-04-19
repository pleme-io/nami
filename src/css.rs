//! CSS parsing and cascade -- lightningcss stylesheets.
//!
//! Parses CSS, resolves the cascade (specificity, inheritance),
//! and produces computed styles for each DOM node.

use crate::dom::Element;
use lightningcss::traits::ToCss;

/// A parsed CSS stylesheet containing rules.
#[derive(Debug, Clone)]
pub struct Stylesheet {
    rules: Vec<Rule>,
}

/// A CSS rule: a selector paired with declarations.
#[derive(Debug, Clone)]
pub struct Rule {
    /// The CSS selector string, e.g. "div.container > p".
    pub selector: String,
    /// The declarations within this rule block.
    pub declarations: Vec<Declaration>,
    /// Specificity of the selector (a, b, c).
    pub specificity: (u32, u32, u32),
}

/// A single CSS property declaration.
#[derive(Debug, Clone)]
pub struct Declaration {
    /// Property name, e.g. "color", "margin-left".
    pub property: String,
    /// Property value as a string, e.g. "red", "16px".
    pub value: String,
    /// Whether this declaration has `!important`.
    pub important: bool,
}

/// Computed style for a DOM element after cascade resolution.
#[derive(Debug, Clone)]
pub struct ComputedStyle {
    pub display: Display,
    pub color: String,
    pub background: String,
    pub font_size: f32,
    pub font_weight: FontWeight,
    pub font_style: FontStyle,
    pub margin: Edges,
    pub padding: Edges,
    pub border_width: Edges,
    pub border_color: String,
    pub border_style: BorderStyle,
    pub width: Dimension,
    pub height: Dimension,
    pub max_width: Dimension,
    pub min_width: Dimension,
    pub text_decoration: TextDecoration,
    pub text_align: TextAlign,
    pub line_height: f32,
    pub overflow: Overflow,
    pub white_space: WhiteSpace,
    pub visibility: Visibility,
    pub list_style_type: ListStyleType,
    pub vertical_align: VerticalAlign,
    pub cursor: Cursor,
}

/// CSS display value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Display {
    Block,
    Inline,
    InlineBlock,
    Flex,
    Grid,
    None,
    ListItem,
    Table,
    TableRow,
    TableCell,
}

/// CSS font-weight.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FontWeight {
    Normal,
    Bold,
    Numeric(u16),
}

/// CSS font-style.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FontStyle {
    Normal,
    Italic,
    Oblique,
}

/// CSS text-decoration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextDecoration {
    None,
    Underline,
    LineThrough,
    Overline,
}

/// CSS text-align.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextAlign {
    Left,
    Right,
    Center,
    Justify,
}

/// CSS overflow.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Overflow {
    Visible,
    Hidden,
    Scroll,
    Auto,
}

/// CSS white-space property.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WhiteSpace {
    Normal,
    NoWrap,
    Pre,
    PreWrap,
    PreLine,
}

/// CSS visibility.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Visibility {
    Visible,
    Hidden,
    Collapse,
}

/// CSS border-style.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BorderStyle {
    None,
    Solid,
    Dashed,
    Dotted,
    Double,
}

/// CSS list-style-type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ListStyleType {
    None,
    Disc,
    Circle,
    Square,
    Decimal,
    LowerAlpha,
    UpperAlpha,
}

/// CSS vertical-align.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VerticalAlign {
    Baseline,
    Top,
    Middle,
    Bottom,
}

/// CSS cursor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cursor {
    Default,
    Pointer,
    Text,
    Move,
    NotAllowed,
}

/// A CSS dimension (length or auto).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Dimension {
    Auto,
    Px(f32),
    Percent(f32),
    Em(f32),
    Rem(f32),
    Vw(f32),
    Vh(f32),
}

/// Edge values for margin, padding, border-width.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Edges {
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
    pub left: f32,
}

impl Edges {
    #[must_use]
    pub const fn uniform(value: f32) -> Self {
        Self {
            top: value,
            right: value,
            bottom: value,
            left: value,
        }
    }

    #[must_use]
    pub const fn zero() -> Self {
        Self::uniform(0.0)
    }
}

impl Default for ComputedStyle {
    fn default() -> Self {
        Self {
            display: Display::Inline,
            color: "#000000".to_string(),
            background: "transparent".to_string(),
            font_size: 16.0,
            font_weight: FontWeight::Normal,
            font_style: FontStyle::Normal,
            margin: Edges::zero(),
            padding: Edges::zero(),
            border_width: Edges::zero(),
            border_color: "#000000".to_string(),
            border_style: BorderStyle::None,
            width: Dimension::Auto,
            height: Dimension::Auto,
            max_width: Dimension::Auto,
            min_width: Dimension::Auto,
            text_decoration: TextDecoration::None,
            text_align: TextAlign::Left,
            line_height: 1.2,
            overflow: Overflow::Visible,
            white_space: WhiteSpace::Normal,
            visibility: Visibility::Visible,
            list_style_type: ListStyleType::Disc,
            vertical_align: VerticalAlign::Baseline,
            cursor: Cursor::Default,
        }
    }
}

impl Stylesheet {
    /// Parse a CSS string into a `Stylesheet`.
    ///
    /// Uses lightningcss for parsing. Unrecognised or invalid rules are
    /// silently skipped to match browser behaviour.
    #[must_use]
    pub fn parse(css: &str) -> Self {
        let mut rules = Vec::new();

        let result = lightningcss::stylesheet::StyleSheet::parse(
            css,
            lightningcss::stylesheet::ParserOptions::default(),
        );

        match result {
            Ok(sheet) => {
                for rule in &sheet.rules.0 {
                    if let lightningcss::rules::CssRule::Style(style_rule) = rule {
                        let selector = style_rule
                            .selectors
                            .to_css_string(lightningcss::printer::PrinterOptions::default())
                            .unwrap_or_default();

                        let mut declarations = Vec::new();

                        for (prop, important) in style_rule.declarations.iter() {
                            let prop_str = prop
                                .to_css_string(
                                    important,
                                    lightningcss::printer::PrinterOptions::default(),
                                )
                                .unwrap_or_default();

                            if let Some((property, value)) = prop_str.split_once(": ") {
                                declarations.push(Declaration {
                                    property: property.to_string(),
                                    value: value.to_string(),
                                    important,
                                });
                            }
                        }

                        let specificity = compute_selector_specificity(&selector);

                        rules.push(Rule {
                            selector,
                            declarations,
                            specificity,
                        });
                    }
                }
            }
            Err(e) => {
                tracing::warn!(error = %e, "CSS parse error, returning empty stylesheet");
            }
        }

        tracing::debug!(rule_count = rules.len(), "parsed stylesheet");
        Self { rules }
    }

    /// Get all rules in the stylesheet.
    #[must_use]
    pub fn rules(&self) -> &[Rule] {
        &self.rules
    }

    /// Look up declarations that match a given element tag.
    ///
    /// Handles: type selectors, universal selector, class selectors,
    /// ID selectors, and simple comma-separated selector lists.
    #[must_use]
    pub fn matching_declarations(&self, tag: &str) -> Vec<&Declaration> {
        let mut result = Vec::new();
        for rule in &self.rules {
            if selector_matches_tag(&rule.selector, tag) {
                result.extend(rule.declarations.iter());
            }
        }
        result
    }

    /// Look up declarations that match a given element (tag + attrs).
    #[must_use]
    pub fn matching_declarations_for_element(&self, elem: &Element) -> Vec<&Declaration> {
        let mut matched: Vec<(&Declaration, (u32, u32, u32))> = Vec::new();
        for rule in &self.rules {
            if selector_matches_element(&rule.selector, elem) {
                for decl in &rule.declarations {
                    matched.push((decl, rule.specificity));
                }
            }
        }
        // Sort by specificity (stable sort preserves source order for equal specificity).
        matched.sort_by_key(|&(_, spec)| spec);
        matched.into_iter().map(|(d, _)| d).collect()
    }

    /// Apply matching declarations to a `ComputedStyle`, returning the result.
    #[must_use]
    pub fn compute_style(&self, tag: &str, base: &ComputedStyle) -> ComputedStyle {
        let mut style = base.clone();
        let declarations = self.matching_declarations(tag);

        for decl in declarations {
            apply_declaration(&mut style, decl);
        }

        style
    }

    /// Compute style for a full element (including class/ID matching).
    #[must_use]
    pub fn compute_style_for_element(&self, elem: &Element, base: &ComputedStyle) -> ComputedStyle {
        let mut style = base.clone();
        let declarations = self.matching_declarations_for_element(elem);

        for decl in declarations {
            apply_declaration(&mut style, decl);
        }

        style
    }
}

/// Check if a selector matches a given tag name (simple matching).
fn selector_matches_tag(selector: &str, tag: &str) -> bool {
    // Handle comma-separated selector lists.
    for part in selector.split(',') {
        let sel = part.trim();
        // Simple selectors only: type, universal.
        let base = sel
            .split_whitespace()
            .next_back()
            .unwrap_or(sel)
            .split('>')
            .next_back()
            .unwrap_or(sel)
            .trim();

        if base == "*" || base == tag {
            return true;
        }
    }
    false
}

/// Check if a selector matches a given element (tag + class + id).
fn selector_matches_element(selector: &str, elem: &Element) -> bool {
    for part in selector.split(',') {
        let sel = part.trim();
        // Get the last simple selector (for descendant/child combinators).
        let last = sel
            .split_whitespace()
            .next_back()
            .unwrap_or(sel)
            .split('>')
            .next_back()
            .unwrap_or(sel)
            .trim();

        if simple_selector_matches(last, elem) {
            return true;
        }
    }
    false
}

/// Check if a simple selector (no combinators) matches an element.
fn simple_selector_matches(selector: &str, elem: &Element) -> bool {
    if selector == "*" {
        return true;
    }

    // Parse the simple selector into tag, classes, and id.
    let mut tag: Option<&str> = None;
    let mut classes: Vec<&str> = Vec::new();
    let mut id: Option<&str> = None;

    let mut remaining = selector;

    // Extract tag name (before any . or #).
    if let Some(dot_pos) = remaining.find('.') {
        if let Some(hash_pos) = remaining.find('#') {
            let first = dot_pos.min(hash_pos);
            if first > 0 {
                tag = Some(&remaining[..first]);
            }
            remaining = &remaining[first..];
        } else {
            if dot_pos > 0 {
                tag = Some(&remaining[..dot_pos]);
            }
            remaining = &remaining[dot_pos..];
        }
    } else if let Some(hash_pos) = remaining.find('#') {
        if hash_pos > 0 {
            tag = Some(&remaining[..hash_pos]);
        }
        remaining = &remaining[hash_pos..];
    } else {
        tag = Some(remaining);
        remaining = "";
    }

    // Parse classes and IDs from remaining.
    let mut i = 0;
    let chars: Vec<char> = remaining.chars().collect();
    while i < chars.len() {
        if chars[i] == '.' {
            let start = i + 1;
            let mut end = start;
            while end < chars.len() && chars[end] != '.' && chars[end] != '#' {
                end += 1;
            }
            if start < end {
                classes.push(&remaining[start..end]);
            }
            i = end;
        } else if chars[i] == '#' {
            let start = i + 1;
            let mut end = start;
            while end < chars.len() && chars[end] != '.' && chars[end] != '#' {
                end += 1;
            }
            if start < end {
                id = Some(&remaining[start..end]);
            }
            i = end;
        } else {
            i += 1;
        }
    }

    // Check tag match.
    if let Some(t) = tag {
        if !t.is_empty() && t != elem.tag {
            return false;
        }
    }

    // Check class match.
    if !classes.is_empty() {
        let elem_classes: Vec<&str> = elem
            .attrs
            .get("class")
            .map(|c| c.split_whitespace().collect())
            .unwrap_or_default();
        for class in &classes {
            if !elem_classes.contains(class) {
                return false;
            }
        }
    }

    // Check ID match.
    if let Some(id_val) = id {
        if elem.attrs.get("id").map(String::as_str) != Some(id_val) {
            return false;
        }
    }

    // Must have matched at least something.
    tag.is_some() || !classes.is_empty() || id.is_some()
}

/// Estimate specificity from a selector string: (id count, class count, type count).
fn compute_selector_specificity(selector: &str) -> (u32, u32, u32) {
    let ids = selector.matches('#').count() as u32;
    let classes = selector.matches('.').count() as u32
        + selector.matches(':').count() as u32
        + selector.matches('[').count() as u32;
    let types = selector
        .split(|c: char| c == '.' || c == '#' || c == ':' || c == '[' || c == ' ' || c == '>')
        .filter(|s| !s.is_empty() && s.chars().next().is_some_and(|c| c.is_ascii_alphabetic()))
        .count() as u32;
    (ids, classes, types)
}

/// Apply a single CSS declaration to a computed style.
fn apply_declaration(style: &mut ComputedStyle, decl: &Declaration) {
    match decl.property.as_str() {
        "color" => style.color = decl.value.clone(),
        "background" | "background-color" => style.background = decl.value.clone(),
        "font-size" => {
            if let Some(px) = parse_px(&decl.value) {
                style.font_size = px;
            }
        }
        "font-weight" => {
            style.font_weight = match decl.value.as_str() {
                "bold" | "700" => FontWeight::Bold,
                "normal" | "400" => FontWeight::Normal,
                other => other
                    .parse::<u16>()
                    .map(FontWeight::Numeric)
                    .unwrap_or(FontWeight::Normal),
            };
        }
        "font-style" => {
            style.font_style = match decl.value.as_str() {
                "italic" => FontStyle::Italic,
                "oblique" => FontStyle::Oblique,
                _ => FontStyle::Normal,
            };
        }
        "display" => {
            style.display = match decl.value.as_str() {
                "block" => Display::Block,
                "inline" => Display::Inline,
                "inline-block" => Display::InlineBlock,
                "flex" => Display::Flex,
                "grid" => Display::Grid,
                "none" => Display::None,
                "list-item" => Display::ListItem,
                "table" => Display::Table,
                "table-row" => Display::TableRow,
                "table-cell" => Display::TableCell,
                _ => style.display,
            };
        }
        "text-decoration" | "text-decoration-line" => {
            style.text_decoration = match decl.value.as_str() {
                "underline" => TextDecoration::Underline,
                "line-through" => TextDecoration::LineThrough,
                "overline" => TextDecoration::Overline,
                "none" => TextDecoration::None,
                _ => style.text_decoration,
            };
        }
        "text-align" => {
            style.text_align = match decl.value.as_str() {
                "left" => TextAlign::Left,
                "right" => TextAlign::Right,
                "center" => TextAlign::Center,
                "justify" => TextAlign::Justify,
                _ => style.text_align,
            };
        }
        "line-height" => {
            if let Ok(val) = decl.value.parse::<f32>() {
                style.line_height = val;
            } else if let Some(px) = parse_px(&decl.value) {
                style.line_height = px / style.font_size;
            }
        }
        "overflow" | "overflow-x" | "overflow-y" => {
            style.overflow = match decl.value.as_str() {
                "visible" => Overflow::Visible,
                "hidden" => Overflow::Hidden,
                "scroll" => Overflow::Scroll,
                "auto" => Overflow::Auto,
                _ => style.overflow,
            };
        }
        "white-space" => {
            style.white_space = match decl.value.as_str() {
                "normal" => WhiteSpace::Normal,
                "nowrap" => WhiteSpace::NoWrap,
                "pre" => WhiteSpace::Pre,
                "pre-wrap" => WhiteSpace::PreWrap,
                "pre-line" => WhiteSpace::PreLine,
                _ => style.white_space,
            };
        }
        "visibility" => {
            style.visibility = match decl.value.as_str() {
                "visible" => Visibility::Visible,
                "hidden" => Visibility::Hidden,
                "collapse" => Visibility::Collapse,
                _ => style.visibility,
            };
        }
        "border-style" => {
            style.border_style = match decl.value.as_str() {
                "none" => BorderStyle::None,
                "solid" => BorderStyle::Solid,
                "dashed" => BorderStyle::Dashed,
                "dotted" => BorderStyle::Dotted,
                "double" => BorderStyle::Double,
                _ => style.border_style,
            };
        }
        "list-style-type" => {
            style.list_style_type = match decl.value.as_str() {
                "none" => ListStyleType::None,
                "disc" => ListStyleType::Disc,
                "circle" => ListStyleType::Circle,
                "square" => ListStyleType::Square,
                "decimal" => ListStyleType::Decimal,
                "lower-alpha" => ListStyleType::LowerAlpha,
                "upper-alpha" => ListStyleType::UpperAlpha,
                _ => style.list_style_type,
            };
        }
        "vertical-align" => {
            style.vertical_align = match decl.value.as_str() {
                "baseline" => VerticalAlign::Baseline,
                "top" => VerticalAlign::Top,
                "middle" => VerticalAlign::Middle,
                "bottom" => VerticalAlign::Bottom,
                _ => style.vertical_align,
            };
        }
        "cursor" => {
            style.cursor = match decl.value.as_str() {
                "default" => Cursor::Default,
                "pointer" => Cursor::Pointer,
                "text" => Cursor::Text,
                "move" => Cursor::Move,
                "not-allowed" => Cursor::NotAllowed,
                _ => style.cursor,
            };
        }
        "margin" => {
            if let Some(px) = parse_px(&decl.value) {
                style.margin = Edges::uniform(px);
            }
        }
        "margin-top" => {
            if let Some(px) = parse_px(&decl.value) {
                style.margin.top = px;
            }
        }
        "margin-right" => {
            if let Some(px) = parse_px(&decl.value) {
                style.margin.right = px;
            }
        }
        "margin-bottom" => {
            if let Some(px) = parse_px(&decl.value) {
                style.margin.bottom = px;
            }
        }
        "margin-left" => {
            if let Some(px) = parse_px(&decl.value) {
                style.margin.left = px;
            }
        }
        "padding" => {
            if let Some(px) = parse_px(&decl.value) {
                style.padding = Edges::uniform(px);
            }
        }
        "padding-top" => {
            if let Some(px) = parse_px(&decl.value) {
                style.padding.top = px;
            }
        }
        "padding-right" => {
            if let Some(px) = parse_px(&decl.value) {
                style.padding.right = px;
            }
        }
        "padding-bottom" => {
            if let Some(px) = parse_px(&decl.value) {
                style.padding.bottom = px;
            }
        }
        "padding-left" => {
            if let Some(px) = parse_px(&decl.value) {
                style.padding.left = px;
            }
        }
        "border" => {
            // Shorthand: "1px solid #000"
            let parts: Vec<&str> = decl.value.split_whitespace().collect();
            if let Some(px) = parts.first().and_then(|p| parse_px(p)) {
                style.border_width = Edges::uniform(px);
            }
            if let Some(s) = parts.get(1) {
                style.border_style = match *s {
                    "solid" => BorderStyle::Solid,
                    "dashed" => BorderStyle::Dashed,
                    "dotted" => BorderStyle::Dotted,
                    "none" => BorderStyle::None,
                    _ => style.border_style,
                };
            }
            if let Some(c) = parts.get(2) {
                style.border_color = (*c).to_string();
            }
        }
        "border-width" => {
            if let Some(px) = parse_px(&decl.value) {
                style.border_width = Edges::uniform(px);
            }
        }
        "border-color" => {
            style.border_color = decl.value.clone();
        }
        "width" => {
            style.width = parse_dimension(&decl.value);
        }
        "height" => {
            style.height = parse_dimension(&decl.value);
        }
        "max-width" => {
            style.max_width = parse_dimension(&decl.value);
        }
        "min-width" => {
            style.min_width = parse_dimension(&decl.value);
        }
        _ => {
            tracing::trace!(
                property = %decl.property,
                value = %decl.value,
                "unhandled CSS property"
            );
        }
    }
}

/// Parse a pixel value from a CSS string like "16px".
fn parse_px(value: &str) -> Option<f32> {
    let trimmed = value.trim();
    if trimmed == "0" {
        return Some(0.0);
    }
    trimmed
        .strip_suffix("px")
        .and_then(|v| v.trim().parse::<f32>().ok())
}

/// Parse a CSS dimension value.
fn parse_dimension(value: &str) -> Dimension {
    let trimmed = value.trim();
    if trimmed == "auto" {
        Dimension::Auto
    } else if trimmed == "0" {
        Dimension::Px(0.0)
    } else if let Some(pct) = trimmed.strip_suffix('%') {
        pct.trim()
            .parse::<f32>()
            .map(Dimension::Percent)
            .unwrap_or(Dimension::Auto)
    } else if let Some(rem) = trimmed.strip_suffix("rem") {
        rem.trim()
            .parse::<f32>()
            .map(Dimension::Rem)
            .unwrap_or(Dimension::Auto)
    } else if let Some(em) = trimmed.strip_suffix("em") {
        em.trim()
            .parse::<f32>()
            .map(Dimension::Em)
            .unwrap_or(Dimension::Auto)
    } else if let Some(vw) = trimmed.strip_suffix("vw") {
        vw.trim()
            .parse::<f32>()
            .map(Dimension::Vw)
            .unwrap_or(Dimension::Auto)
    } else if let Some(vh) = trimmed.strip_suffix("vh") {
        vh.trim()
            .parse::<f32>()
            .map(Dimension::Vh)
            .unwrap_or(Dimension::Auto)
    } else if let Some(px) = parse_px(trimmed) {
        Dimension::Px(px)
    } else {
        Dimension::Auto
    }
}

/// Resolve a `Dimension` to pixels given context.
#[must_use]
pub fn resolve_dimension(dim: Dimension, container_px: f32, font_size_px: f32) -> Option<f32> {
    match dim {
        Dimension::Auto => None,
        Dimension::Px(px) => Some(px),
        Dimension::Percent(pct) => Some(container_px * pct / 100.0),
        Dimension::Em(em) => Some(em * font_size_px),
        Dimension::Rem(rem) => Some(rem * 16.0), // Root font size is 16px.
        Dimension::Vw(vw) => Some(container_px * vw / 100.0), // Approximate.
        Dimension::Vh(vh) => Some(container_px * vh / 100.0),
    }
}

/// Compute cascaded style for a DOM element using all stylesheets.
pub fn cascade_style(
    elem: &Element,
    stylesheets: &[Stylesheet],
    parent_style: &ComputedStyle,
) -> ComputedStyle {
    let mut style = default_style_for_tag(&elem.tag);

    // Inherit inheritable properties from parent.
    style.color = parent_style.color.clone();
    style.font_size = parent_style.font_size;
    style.font_weight = parent_style.font_weight;
    style.font_style = parent_style.font_style;
    style.line_height = parent_style.line_height;
    style.text_align = parent_style.text_align;
    style.white_space = parent_style.white_space;
    style.list_style_type = parent_style.list_style_type;
    style.cursor = parent_style.cursor;
    style.visibility = parent_style.visibility;

    // Re-apply tag defaults that override inherited values.
    apply_tag_defaults(&mut style, &elem.tag);

    // Apply stylesheet rules.
    for sheet in stylesheets {
        let decls = sheet.matching_declarations_for_element(elem);
        for decl in decls {
            apply_declaration(&mut style, decl);
        }
    }

    // Apply inline style attribute.
    if let Some(inline_css) = elem.attrs.get("style") {
        let inline_sheet = Stylesheet::parse(&format!("* {{ {inline_css} }}"));
        for rule in inline_sheet.rules() {
            for decl in &rule.declarations {
                apply_declaration(&mut style, decl);
            }
        }
    }

    style
}

/// Return the default (user-agent) style for common HTML tags.
#[must_use]
pub fn default_style_for_tag(tag: &str) -> ComputedStyle {
    let mut style = ComputedStyle::default();

    match tag {
        "div" | "p" | "section" | "article" | "main" | "header" | "footer" | "nav" | "aside"
        | "form" | "fieldset" | "address" | "figure" | "figcaption" | "details" | "summary" => {
            style.display = Display::Block;
        }
        "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => {
            style.display = Display::Block;
        }
        "ul" | "ol" => {
            style.display = Display::Block;
            style.padding.left = 40.0;
        }
        "li" => {
            style.display = Display::ListItem;
        }
        "blockquote" => {
            style.display = Display::Block;
            style.margin = Edges {
                top: 16.0,
                right: 40.0,
                bottom: 16.0,
                left: 40.0,
            };
        }
        "pre" => {
            style.display = Display::Block;
            style.white_space = WhiteSpace::Pre;
        }
        "table" => {
            style.display = Display::Table;
        }
        "tr" => {
            style.display = Display::TableRow;
        }
        "td" | "th" => {
            style.display = Display::TableCell;
        }
        "hr" => {
            style.display = Display::Block;
            style.border_style = BorderStyle::Solid;
            style.border_width = Edges {
                top: 1.0,
                right: 0.0,
                bottom: 0.0,
                left: 0.0,
            };
            style.margin = Edges {
                top: 8.0,
                right: 0.0,
                bottom: 8.0,
                left: 0.0,
            };
        }
        "html" | "body" => {
            style.display = Display::Block;
        }
        "head" | "script" | "style" | "meta" | "link" | "title" | "noscript" => {
            style.display = Display::None;
        }
        "a" => {
            style.display = Display::Inline;
            style.color = "#5e81ac".to_string();
            style.text_decoration = TextDecoration::Underline;
            style.cursor = Cursor::Pointer;
        }
        "strong" | "b" => {
            style.font_weight = FontWeight::Bold;
        }
        "em" | "i" => {
            style.font_style = FontStyle::Italic;
        }
        "code" | "kbd" | "samp" | "tt" => {
            style.white_space = WhiteSpace::PreWrap;
        }
        "br" => {
            style.display = Display::Block;
        }
        "img" => {
            style.display = Display::Inline;
        }
        _ => {
            style.display = Display::Inline;
        }
    }

    // Heading font sizes.
    apply_tag_defaults(&mut style, tag);

    style
}

/// Apply tag-specific defaults (font size, weight) that override inheritance.
fn apply_tag_defaults(style: &mut ComputedStyle, tag: &str) {
    match tag {
        "h1" => {
            style.font_size = 32.0;
            style.font_weight = FontWeight::Bold;
            style.margin = Edges {
                top: 21.0,
                right: 0.0,
                bottom: 21.0,
                left: 0.0,
            };
        }
        "h2" => {
            style.font_size = 24.0;
            style.font_weight = FontWeight::Bold;
            style.margin = Edges {
                top: 19.0,
                right: 0.0,
                bottom: 19.0,
                left: 0.0,
            };
        }
        "h3" => {
            style.font_size = 18.7;
            style.font_weight = FontWeight::Bold;
            style.margin = Edges {
                top: 18.0,
                right: 0.0,
                bottom: 18.0,
                left: 0.0,
            };
        }
        "h4" => {
            style.font_size = 16.0;
            style.font_weight = FontWeight::Bold;
            style.margin = Edges {
                top: 21.0,
                right: 0.0,
                bottom: 21.0,
                left: 0.0,
            };
        }
        "h5" => {
            style.font_size = 13.3;
            style.font_weight = FontWeight::Bold;
        }
        "h6" => {
            style.font_size = 10.7;
            style.font_weight = FontWeight::Bold;
        }
        "strong" | "b" => {
            style.font_weight = FontWeight::Bold;
        }
        "em" | "i" => {
            style.font_style = FontStyle::Italic;
        }
        "p" => {
            style.margin = Edges {
                top: 16.0,
                right: 0.0,
                bottom: 16.0,
                left: 0.0,
            };
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_empty_css() {
        let sheet = Stylesheet::parse("");
        assert!(sheet.rules().is_empty());
    }

    #[test]
    fn parse_single_rule() {
        let css = "p { color: red; font-size: 16px; }";
        let sheet = Stylesheet::parse(css);
        assert_eq!(sheet.rules().len(), 1);
        assert_eq!(sheet.rules()[0].selector, "p");
        assert!(!sheet.rules()[0].declarations.is_empty());
    }

    #[test]
    fn parse_multiple_rules() {
        let css = "h1 { font-size: 24px; } p { color: blue; } a { text-decoration: underline; }";
        let sheet = Stylesheet::parse(css);
        assert_eq!(sheet.rules().len(), 3);
    }

    #[test]
    fn matching_declarations_type_selector() {
        let css = "p { color: red; } div { color: blue; }";
        let sheet = Stylesheet::parse(css);

        let p_decls = sheet.matching_declarations("p");
        assert!(!p_decls.is_empty());
        assert!(p_decls.iter().any(|d| d.property == "color"));

        let div_decls = sheet.matching_declarations("div");
        assert!(!div_decls.is_empty());
        assert!(div_decls.iter().any(|d| d.property == "color"));
    }

    #[test]
    fn matching_declarations_universal_selector() {
        let css = "* { margin: 0px; }";
        let sheet = Stylesheet::parse(css);

        let decls = sheet.matching_declarations("anything");
        assert!(!decls.is_empty());
    }

    #[test]
    fn matching_declarations_no_match() {
        let css = "p { color: red; }";
        let sheet = Stylesheet::parse(css);

        let decls = sheet.matching_declarations("div");
        assert!(decls.is_empty());
    }

    #[test]
    fn compute_style_applies_color() {
        let css = "p { color: red; }";
        let sheet = Stylesheet::parse(css);
        let base = ComputedStyle::default();

        let computed = sheet.compute_style("p", &base);
        assert_eq!(computed.color, "red");
    }

    #[test]
    fn compute_style_applies_font_size() {
        let css = "h1 { font-size: 24px; }";
        let sheet = Stylesheet::parse(css);
        let base = ComputedStyle::default();

        let computed = sheet.compute_style("h1", &base);
        assert!((computed.font_size - 24.0).abs() < f32::EPSILON);
    }

    #[test]
    fn compute_style_applies_display() {
        let css = "span { display: block; }";
        let sheet = Stylesheet::parse(css);
        let base = ComputedStyle::default();

        let computed = sheet.compute_style("span", &base);
        assert_eq!(computed.display, Display::Block);
    }

    #[test]
    fn compute_style_applies_font_weight() {
        let css = "strong { font-weight: bold; }";
        let sheet = Stylesheet::parse(css);
        let base = ComputedStyle::default();

        let computed = sheet.compute_style("strong", &base);
        assert_eq!(computed.font_weight, FontWeight::Bold);
    }

    #[test]
    fn compute_style_no_match_returns_base() {
        let css = "p { color: red; }";
        let sheet = Stylesheet::parse(css);
        let base = ComputedStyle::default();

        let computed = sheet.compute_style("div", &base);
        assert_eq!(computed.color, base.color);
    }

    #[test]
    fn parse_px_values() {
        assert_eq!(parse_px("16px"), Some(16.0));
        assert_eq!(parse_px("0"), Some(0.0));
        assert_eq!(parse_px("3.5px"), Some(3.5));
        assert_eq!(parse_px("auto"), None);
        assert_eq!(parse_px("red"), None);
    }

    #[test]
    fn parse_dimension_values() {
        assert_eq!(parse_dimension("auto"), Dimension::Auto);
        assert_eq!(parse_dimension("100px"), Dimension::Px(100.0));
        assert_eq!(parse_dimension("50%"), Dimension::Percent(50.0));
        assert_eq!(parse_dimension("2em"), Dimension::Em(2.0));
        assert_eq!(parse_dimension("1.5rem"), Dimension::Rem(1.5));
    }

    #[test]
    fn edges_uniform() {
        let e = Edges::uniform(10.0);
        assert!((e.top - 10.0).abs() < f32::EPSILON);
        assert!((e.right - 10.0).abs() < f32::EPSILON);
        assert!((e.bottom - 10.0).abs() < f32::EPSILON);
        assert!((e.left - 10.0).abs() < f32::EPSILON);
    }

    #[test]
    fn edges_zero() {
        let e = Edges::zero();
        assert!(e.top.abs() < f32::EPSILON);
    }

    #[test]
    fn default_computed_style() {
        let s = ComputedStyle::default();
        assert_eq!(s.display, Display::Inline);
        assert_eq!(s.color, "#000000");
        assert_eq!(s.background, "transparent");
        assert!((s.font_size - 16.0).abs() < f32::EPSILON);
        assert_eq!(s.font_weight, FontWeight::Normal);
        assert_eq!(s.text_decoration, TextDecoration::None);
        assert_eq!(s.text_align, TextAlign::Left);
    }

    #[test]
    fn parse_invalid_css_returns_empty() {
        let css = "this is { not: valid {{ css }}}}";
        let sheet = Stylesheet::parse(css);
        let _ = sheet.rules();
    }

    #[test]
    fn class_selector_matching() {
        let elem = Element {
            tag: "div".to_string(),
            attrs: [("class".to_string(), "container active".to_string())]
                .into_iter()
                .collect(),
            children: Vec::new(),
        };
        assert!(simple_selector_matches(".container", &elem));
        assert!(simple_selector_matches("div.container", &elem));
        assert!(simple_selector_matches(".active", &elem));
        assert!(!simple_selector_matches(".missing", &elem));
    }

    #[test]
    fn id_selector_matching() {
        let elem = Element {
            tag: "div".to_string(),
            attrs: [("id".to_string(), "main".to_string())]
                .into_iter()
                .collect(),
            children: Vec::new(),
        };
        assert!(simple_selector_matches("#main", &elem));
        assert!(simple_selector_matches("div#main", &elem));
        assert!(!simple_selector_matches("#other", &elem));
    }

    #[test]
    fn cascade_inherits_color() {
        let parent = ComputedStyle {
            color: "red".to_string(),
            ..ComputedStyle::default()
        };
        let elem = Element {
            tag: "span".to_string(),
            attrs: std::collections::HashMap::new(),
            children: Vec::new(),
        };
        let result = cascade_style(&elem, &[], &parent);
        assert_eq!(result.color, "red");
    }
}
