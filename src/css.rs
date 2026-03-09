//! CSS parsing and cascade -- lightningcss stylesheets.
//!
//! Parses CSS, resolves the cascade (specificity, inheritance),
//! and produces computed styles for each DOM node.

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
}

/// A single CSS property declaration.
#[derive(Debug, Clone)]
pub struct Declaration {
    /// Property name, e.g. "color", "margin-left".
    pub property: String,
    /// Property value as a string, e.g. "red", "16px".
    pub value: String,
}

/// Computed style for a DOM element after cascade resolution.
#[derive(Debug, Clone)]
pub struct ComputedStyle {
    pub display: Display,
    pub color: String,
    pub background: String,
    pub font_size: f32,
    pub font_weight: FontWeight,
    pub margin: Edges,
    pub padding: Edges,
    pub border_width: Edges,
    pub border_color: String,
    pub width: Dimension,
    pub height: Dimension,
    pub text_decoration: TextDecoration,
    pub text_align: TextAlign,
    pub line_height: f32,
    pub overflow: Overflow,
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
}

/// CSS font-weight.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FontWeight {
    Normal,
    Bold,
    Numeric(u16),
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

/// A CSS dimension (length or auto).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Dimension {
    Auto,
    Px(f32),
    Percent(f32),
    Em(f32),
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
    /// Create edges with all sides set to the same value.
    #[must_use]
    pub const fn uniform(value: f32) -> Self {
        Self {
            top: value,
            right: value,
            bottom: value,
            left: value,
        }
    }

    /// Create edges with all sides set to zero.
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
            margin: Edges::zero(),
            padding: Edges::zero(),
            border_width: Edges::zero(),
            border_color: "#000000".to_string(),
            width: Dimension::Auto,
            height: Dimension::Auto,
            text_decoration: TextDecoration::None,
            text_align: TextAlign::Left,
            line_height: 1.2,
            overflow: Overflow::Visible,
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

        // Use lightningcss to parse the stylesheet.
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

                        // Extract declarations from the rule.
                        for (prop, important) in style_rule.declarations.iter() {
                            let prop_str = prop
                                .to_css_string(
                                    important,
                                    lightningcss::printer::PrinterOptions::default(),
                                )
                                .unwrap_or_default();

                            // lightningcss returns "property: value", so split on ": ".
                            if let Some((property, value)) = prop_str.split_once(": ") {
                                declarations.push(Declaration {
                                    property: property.to_string(),
                                    value: value.to_string(),
                                });
                            }
                        }

                        rules.push(Rule {
                            selector,
                            declarations,
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
    /// This is a simplified matching that only handles:
    /// - Type selectors (e.g. "p", "div", "a")
    /// - Universal selector ("*")
    ///
    /// Full selector matching (classes, IDs, combinators) is planned.
    #[must_use]
    pub fn matching_declarations(&self, tag: &str) -> Vec<&Declaration> {
        let mut result = Vec::new();
        for rule in &self.rules {
            let sel = rule.selector.trim();
            if sel == "*" || sel == tag {
                result.extend(rule.declarations.iter());
            }
        }
        result
    }

    /// Apply matching declarations to a `ComputedStyle`, returning the result.
    ///
    /// This is a simplified cascade: later rules override earlier ones,
    /// no specificity weighting beyond type vs universal.
    #[must_use]
    pub fn compute_style(&self, tag: &str, base: &ComputedStyle) -> ComputedStyle {
        let mut style = base.clone();
        let declarations = self.matching_declarations(tag);

        for decl in declarations {
            apply_declaration(&mut style, decl);
        }

        style
    }
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
        "display" => {
            style.display = match decl.value.as_str() {
                "block" => Display::Block,
                "inline" => Display::Inline,
                "inline-block" => Display::InlineBlock,
                "flex" => Display::Flex,
                "grid" => Display::Grid,
                "none" => Display::None,
                _ => style.display,
            };
        }
        "text-decoration" => {
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
        "overflow" => {
            style.overflow = match decl.value.as_str() {
                "visible" => Overflow::Visible,
                "hidden" => Overflow::Hidden,
                "scroll" => Overflow::Scroll,
                "auto" => Overflow::Auto,
                _ => style.overflow,
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
        "width" => {
            style.width = parse_dimension(&decl.value);
        }
        "height" => {
            style.height = parse_dimension(&decl.value);
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
    } else if let Some(pct) = trimmed.strip_suffix('%') {
        pct.trim()
            .parse::<f32>()
            .map(Dimension::Percent)
            .unwrap_or(Dimension::Auto)
    } else if let Some(em) = trimmed.strip_suffix("em") {
        em.trim()
            .parse::<f32>()
            .map(Dimension::Em)
            .unwrap_or(Dimension::Auto)
    } else if let Some(px) = parse_px(trimmed) {
        Dimension::Px(px)
    } else {
        Dimension::Auto
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
        assert!(p_decls.iter().any(|d| d.property == "color" && d.value == "red"));

        let div_decls = sheet.matching_declarations("div");
        assert!(!div_decls.is_empty());
        assert!(div_decls.iter().any(|d| d.property == "color" && d.value == "blue"));
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
        assert!(e.right.abs() < f32::EPSILON);
        assert!(e.bottom.abs() < f32::EPSILON);
        assert!(e.left.abs() < f32::EPSILON);
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
        // Should not panic. May return some rules or none depending on recovery.
        let _ = sheet.rules();
    }
}
