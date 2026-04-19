//! Rendering module -- page content to displayable text.
//!
//! Converts the layout tree into a sequence of styled text lines for display.
//! In the future this will use garasu for GPU rendering; for now it produces
//! styled terminal output that can be printed or piped.
//!
//! # Architecture
//!
//! The rendering pipeline walks the layout tree and collects styled text spans:
//!
//! ```text
//! LayoutTree (positioned boxes)
//!       |
//!       v
//! StyledLine[] (text with colors and decorations)
//!       |
//!       v
//! Terminal output (ANSI escape codes) or GPU (garasu)
//! ```

use crate::css::{ComputedStyle, FontWeight, TextDecoration};
use crate::dom::{Document, Node, NodeKind};
use crate::layout::LayoutTree;

/// A styled text span within a line.
#[derive(Debug, Clone)]
pub struct StyledSpan {
    /// The text content.
    pub text: String,
    /// Foreground color as hex string (e.g., "#eceff4").
    pub fg_color: String,
    /// Background color (or "transparent").
    pub bg_color: String,
    /// Whether text is bold.
    pub bold: bool,
    /// Whether text is italic.
    pub italic: bool,
    /// Whether text is underlined.
    pub underline: bool,
    /// Whether text is struck through.
    pub strikethrough: bool,
    /// Whether this span is a link.
    pub is_link: bool,
    /// Link href if this is a link.
    pub href: Option<String>,
}

/// A line of styled text.
#[derive(Debug, Clone)]
pub struct StyledLine {
    /// Spans within this line.
    pub spans: Vec<StyledSpan>,
    /// Y position in pixels (from layout).
    pub y: f32,
    /// Indentation level.
    pub indent: usize,
}

/// A rendered page ready for display.
#[derive(Debug)]
pub struct RenderedPage {
    /// Lines of styled text.
    pub lines: Vec<StyledLine>,
    /// The page title.
    pub title: String,
    /// Total content height in lines.
    pub total_lines: usize,
}

/// Render a document with styles into styled text lines.
///
/// This walks the DOM tree (not the layout tree) to produce readable
/// text output with structural formatting. The layout tree is used
/// for positional information when available.
#[must_use]
pub fn render_document(
    doc: &Document,
    _layout: Option<&LayoutTree>,
    viewport_width: u32,
) -> RenderedPage {
    let title = doc.title().unwrap_or("Untitled").to_string();
    let mut lines: Vec<StyledLine> = Vec::new();
    let default_style = ComputedStyle {
        color: "#eceff4".to_string(),
        ..ComputedStyle::default()
    };

    // Walk the DOM and build styled lines.
    render_node(
        &doc.root,
        &default_style,
        &mut lines,
        0,
        viewport_width,
        false,
        None,
    );

    let total_lines = lines.len();

    RenderedPage {
        lines,
        title,
        total_lines,
    }
}

/// Recursively render a DOM node into styled lines.
fn render_node(
    node: &Node,
    parent_style: &ComputedStyle,
    lines: &mut Vec<StyledLine>,
    indent: usize,
    viewport_width: u32,
    in_link: bool,
    link_href: Option<&str>,
) {
    match &node.kind {
        NodeKind::Text(text) => {
            let collapsed: String = text.split_whitespace().collect::<Vec<_>>().join(" ");
            if collapsed.is_empty() {
                return;
            }

            let span = StyledSpan {
                text: collapsed,
                fg_color: parent_style.color.clone(),
                bg_color: parent_style.background.clone(),
                bold: parent_style.font_weight == FontWeight::Bold,
                italic: parent_style.font_style == crate::css::FontStyle::Italic,
                underline: parent_style.text_decoration == TextDecoration::Underline,
                strikethrough: parent_style.text_decoration == TextDecoration::LineThrough,
                is_link: in_link,
                href: link_href.map(String::from),
            };

            // Append to the current line or start a new one.
            if let Some(last_line) = lines.last_mut() {
                last_line.spans.push(span);
            } else {
                lines.push(StyledLine {
                    spans: vec![span],
                    y: 0.0,
                    indent,
                });
            }
        }
        NodeKind::Element(elem) => {
            // Skip non-visible elements.
            if matches!(
                elem.tag.as_str(),
                "script" | "style" | "head" | "meta" | "link" | "title" | "noscript"
            ) {
                return;
            }

            let is_block = matches!(
                elem.tag.as_str(),
                "div"
                    | "p"
                    | "h1"
                    | "h2"
                    | "h3"
                    | "h4"
                    | "h5"
                    | "h6"
                    | "section"
                    | "article"
                    | "main"
                    | "header"
                    | "footer"
                    | "nav"
                    | "aside"
                    | "ul"
                    | "ol"
                    | "blockquote"
                    | "pre"
                    | "form"
                    | "table"
                    | "tr"
                    | "dl"
                    | "figure"
                    | "figcaption"
                    | "details"
                    | "summary"
            );

            // Compute style for this element.
            let mut style = parent_style.clone();
            match elem.tag.as_str() {
                "h1" => {
                    style.font_size = 32.0;
                    style.font_weight = FontWeight::Bold;
                }
                "h2" => {
                    style.font_size = 24.0;
                    style.font_weight = FontWeight::Bold;
                }
                "h3" => {
                    style.font_size = 18.7;
                    style.font_weight = FontWeight::Bold;
                }
                "h4" | "h5" | "h6" => {
                    style.font_weight = FontWeight::Bold;
                }
                "strong" | "b" => {
                    style.font_weight = FontWeight::Bold;
                }
                "em" | "i" => {
                    style.font_style = crate::css::FontStyle::Italic;
                }
                "a" => {
                    style.color = "#5e81ac".to_string();
                    style.text_decoration = TextDecoration::Underline;
                }
                "code" | "kbd" | "samp" => {
                    style.background = "#3b4252".to_string();
                }
                _ => {}
            }

            // Block elements start a new line.
            if is_block {
                // Ensure previous content is on its own line.
                if let Some(last) = lines.last() {
                    if !last.spans.is_empty() {
                        lines.push(StyledLine {
                            spans: Vec::new(),
                            y: 0.0,
                            indent,
                        });
                    }
                }
            }

            // Heading prefix.
            match elem.tag.as_str() {
                "h1" => {
                    push_prefix(lines, indent, "# ", &style);
                }
                "h2" => {
                    push_prefix(lines, indent, "## ", &style);
                }
                "h3" => {
                    push_prefix(lines, indent, "### ", &style);
                }
                "h4" => {
                    push_prefix(lines, indent, "#### ", &style);
                }
                "h5" => {
                    push_prefix(lines, indent, "##### ", &style);
                }
                "h6" => {
                    push_prefix(lines, indent, "###### ", &style);
                }
                "li" => {
                    push_prefix(lines, indent, "  - ", &style);
                }
                "blockquote" => {
                    push_prefix(lines, indent, "> ", &style);
                }
                "hr" => {
                    let width = viewport_width.min(80) as usize;
                    lines.push(StyledLine {
                        spans: vec![StyledSpan {
                            text: "-".repeat(width),
                            fg_color: "#4c566a".to_string(),
                            bg_color: "transparent".to_string(),
                            bold: false,
                            italic: false,
                            underline: false,
                            strikethrough: false,
                            is_link: false,
                            href: None,
                        }],
                        y: 0.0,
                        indent,
                    });
                    return;
                }
                "br" => {
                    lines.push(StyledLine {
                        spans: Vec::new(),
                        y: 0.0,
                        indent,
                    });
                    return;
                }
                "img" => {
                    let alt = elem.attrs.get("alt").map(String::as_str).unwrap_or("image");
                    push_prefix(lines, indent, &format!("[img: {alt}]"), &style);
                    return;
                }
                _ => {}
            }

            // Link tracking.
            let is_link_elem = elem.tag == "a";
            let href = if is_link_elem {
                elem.attrs.get("href").map(String::as_str)
            } else {
                link_href
            };

            let child_indent = if elem.tag == "ul" || elem.tag == "ol" {
                indent + 1
            } else {
                indent
            };

            // Recurse into children.
            for child in &elem.children {
                render_node(
                    child,
                    &style,
                    lines,
                    child_indent,
                    viewport_width,
                    in_link || is_link_elem,
                    href,
                );
            }

            // After link text, show the URL.
            if is_link_elem {
                if let Some(h) = elem.attrs.get("href") {
                    if let Some(last_line) = lines.last_mut() {
                        last_line.spans.push(StyledSpan {
                            text: format!(" [{h}]"),
                            fg_color: "#4c566a".to_string(),
                            bg_color: "transparent".to_string(),
                            bold: false,
                            italic: false,
                            underline: false,
                            strikethrough: false,
                            is_link: true,
                            href: Some(h.clone()),
                        });
                    }
                }
            }

            // Block elements end with a newline.
            if is_block {
                lines.push(StyledLine {
                    spans: Vec::new(),
                    y: 0.0,
                    indent,
                });
            }
        }
        NodeKind::Comment(_) => {}
    }
}

/// Push a prefix span onto the current or new line.
fn push_prefix(lines: &mut Vec<StyledLine>, indent: usize, prefix: &str, style: &ComputedStyle) {
    let span = StyledSpan {
        text: prefix.to_string(),
        fg_color: style.color.clone(),
        bg_color: "transparent".to_string(),
        bold: style.font_weight == FontWeight::Bold,
        italic: false,
        underline: false,
        strikethrough: false,
        is_link: false,
        href: None,
    };

    if lines.is_empty() || !lines.last().unwrap().spans.is_empty() {
        lines.push(StyledLine {
            spans: vec![span],
            y: 0.0,
            indent,
        });
    } else {
        lines.last_mut().unwrap().spans.push(span);
    }
}

/// Convert a `RenderedPage` to plain text (no ANSI codes).
#[must_use]
pub fn to_plain_text(page: &RenderedPage) -> String {
    let mut output = String::new();
    for line in &page.lines {
        for _ in 0..line.indent {
            output.push_str("  ");
        }
        for span in &line.spans {
            output.push_str(&span.text);
        }
        output.push('\n');
    }
    output
}

/// Convert a `RenderedPage` to ANSI-colored text for terminal display.
#[must_use]
pub fn to_ansi_text(page: &RenderedPage) -> String {
    let mut output = String::new();
    for line in &page.lines {
        for _ in 0..line.indent {
            output.push_str("  ");
        }
        for span in &line.spans {
            let mut codes = Vec::new();
            if span.bold {
                codes.push("1");
            }
            if span.italic {
                codes.push("3");
            }
            if span.underline {
                codes.push("4");
            }
            if span.strikethrough {
                codes.push("9");
            }

            // Convert hex color to ANSI 256 color (approximate).
            if let Some(ansi) = hex_to_ansi256(&span.fg_color) {
                output.push_str(&format!("\x1b[38;5;{ansi}m"));
            }

            if !codes.is_empty() {
                output.push_str(&format!("\x1b[{}m", codes.join(";")));
            }

            output.push_str(&span.text);

            if !codes.is_empty() || hex_to_ansi256(&span.fg_color).is_some() {
                output.push_str("\x1b[0m");
            }
        }
        output.push('\n');
    }
    output
}

/// Convert a hex color to an ANSI 256 color code (rough approximation).
fn hex_to_ansi256(hex: &str) -> Option<u8> {
    let hex = hex.strip_prefix('#')?;
    if hex.len() < 6 {
        return None;
    }
    let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
    let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
    let b = u8::from_str_radix(&hex[4..6], 16).ok()?;

    // Map to 256-color palette.
    // 16-231: 6x6x6 color cube.
    let r_idx = ((f32::from(r) / 255.0) * 5.0).round() as u8;
    let g_idx = ((f32::from(g) / 255.0) * 5.0).round() as u8;
    let b_idx = ((f32::from(b) / 255.0) * 5.0).round() as u8;

    Some(16 + 36 * r_idx + 6 * g_idx + b_idx)
}

/// Render a status bar line.
#[must_use]
pub fn render_status_bar(
    url: &str,
    mode: &str,
    tab_info: &str,
    blocked_count: u64,
    loading: bool,
) -> String {
    let status = if loading { "Loading..." } else { "" };
    let blocked = if blocked_count > 0 {
        format!(" [{blocked_count} blocked]")
    } else {
        String::new()
    };

    format!("\x1b[7m {mode} | {url} {status}{blocked} | {tab_info} \x1b[0m")
}

/// Render a tab bar line.
#[must_use]
pub fn render_tab_bar(tabs: &[(&str, bool)]) -> String {
    let mut out = String::new();
    for (i, (title, active)) in tabs.iter().enumerate() {
        if *active {
            out.push_str(&format!(
                "\x1b[7m [{}: {}] \x1b[0m",
                i + 1,
                truncate(title, 20)
            ));
        } else {
            out.push_str(&format!("  {}: {}  ", i + 1, truncate(title, 20)));
        }
    }
    out
}

/// Truncate a string to a max length with ellipsis.
fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        let end = max.saturating_sub(3);
        let trimmed = s[..end].trim_end();
        format!("{trimmed}...")
    }
}

/// Render the "about:blank" page.
#[must_use]
pub fn render_blank_page() -> RenderedPage {
    let lines = vec![
        StyledLine {
            spans: vec![],
            y: 0.0,
            indent: 0,
        },
        StyledLine {
            spans: vec![StyledSpan {
                text: "  Nami Browser".to_string(),
                fg_color: "#88c0d0".to_string(),
                bg_color: "transparent".to_string(),
                bold: true,
                italic: false,
                underline: false,
                strikethrough: false,
                is_link: false,
                href: None,
            }],
            y: 0.0,
            indent: 0,
        },
        StyledLine {
            spans: vec![StyledSpan {
                text: "  GPU-rendered TUI browser".to_string(),
                fg_color: "#4c566a".to_string(),
                bg_color: "transparent".to_string(),
                bold: false,
                italic: true,
                underline: false,
                strikethrough: false,
                is_link: false,
                href: None,
            }],
            y: 0.0,
            indent: 0,
        },
        StyledLine {
            spans: vec![],
            y: 0.0,
            indent: 0,
        },
        StyledLine {
            spans: vec![StyledSpan {
                text: "  Type 'o' to open a URL, ':help' for commands".to_string(),
                fg_color: "#eceff4".to_string(),
                bg_color: "transparent".to_string(),
                bold: false,
                italic: false,
                underline: false,
                strikethrough: false,
                is_link: false,
                href: None,
            }],
            y: 0.0,
            indent: 0,
        },
        StyledLine {
            spans: vec![],
            y: 0.0,
            indent: 0,
        },
        StyledLine {
            spans: vec![StyledSpan {
                text: "  Keybindings:".to_string(),
                fg_color: "#88c0d0".to_string(),
                bg_color: "transparent".to_string(),
                bold: true,
                italic: false,
                underline: false,
                strikethrough: false,
                is_link: false,
                href: None,
            }],
            y: 0.0,
            indent: 0,
        },
        help_line("  j/k", "Scroll down/up"),
        help_line("  d/u", "Half-page down/up"),
        help_line("  gg/G", "Top/bottom of page"),
        help_line("  o/O", "Open URL / Open in new tab"),
        help_line("  f/F", "Follow link / Follow in new tab"),
        help_line("  H/L", "Go back/forward"),
        help_line("  t/x", "New tab / Close tab"),
        help_line("  gt/gT", "Next/previous tab"),
        help_line("  /", "Search on page"),
        help_line("  :", "Command mode"),
        help_line("  yy", "Copy current URL"),
        help_line("  B", "Toggle bookmark"),
        help_line("  :q", "Quit"),
    ];

    let total_lines = lines.len();
    RenderedPage {
        lines,
        title: "New Tab".to_string(),
        total_lines,
    }
}

fn help_line(key: &str, desc: &str) -> StyledLine {
    StyledLine {
        spans: vec![
            StyledSpan {
                text: format!("{key:<12}"),
                fg_color: "#a3be8c".to_string(),
                bg_color: "transparent".to_string(),
                bold: true,
                italic: false,
                underline: false,
                strikethrough: false,
                is_link: false,
                href: None,
            },
            StyledSpan {
                text: desc.to_string(),
                fg_color: "#eceff4".to_string(),
                bg_color: "transparent".to_string(),
                bold: false,
                italic: false,
                underline: false,
                strikethrough: false,
                is_link: false,
                href: None,
            },
        ],
        y: 0.0,
        indent: 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_simple_page() {
        let doc = Document::parse("<html><body><h1>Hello</h1><p>World</p></body></html>");
        let page = render_document(&doc, None, 80);
        assert!(!page.lines.is_empty());
        assert_eq!(page.title, "Untitled");

        let text = to_plain_text(&page);
        assert!(text.contains("Hello"));
        assert!(text.contains("World"));
    }

    #[test]
    fn render_with_links() {
        let doc =
            Document::parse(r#"<html><body><a href="https://example.com">Click</a></body></html>"#);
        let page = render_document(&doc, None, 80);
        let text = to_plain_text(&page);
        assert!(text.contains("Click"));
        assert!(text.contains("example.com"));
    }

    #[test]
    fn render_headings() {
        let doc =
            Document::parse("<html><body><h1>Big</h1><h2>Medium</h2><h3>Small</h3></body></html>");
        let page = render_document(&doc, None, 80);
        let text = to_plain_text(&page);
        assert!(text.contains("# Big"));
        assert!(text.contains("## Medium"));
        assert!(text.contains("### Small"));
    }

    #[test]
    fn render_blank_page_has_content() {
        let page = render_blank_page();
        assert!(!page.lines.is_empty());
        let text = to_plain_text(&page);
        assert!(text.contains("Nami Browser"));
    }

    #[test]
    fn plain_text_output() {
        let doc = Document::parse("<html><body><p>Test paragraph</p></body></html>");
        let page = render_document(&doc, None, 80);
        let text = to_plain_text(&page);
        assert!(text.contains("Test paragraph"));
        // Should not contain ANSI codes.
        assert!(!text.contains("\x1b["));
    }

    #[test]
    fn ansi_text_output() {
        let doc = Document::parse("<html><body><strong>Bold</strong></body></html>");
        let page = render_document(&doc, None, 80);
        let text = to_ansi_text(&page);
        // Should contain ANSI bold code.
        assert!(text.contains("\x1b[1m"));
    }

    #[test]
    fn status_bar_rendering() {
        let bar = render_status_bar("https://example.com", "NORMAL", "1/3", 5, false);
        assert!(bar.contains("NORMAL"));
        assert!(bar.contains("example.com"));
        assert!(bar.contains("5 blocked"));
    }

    #[test]
    fn tab_bar_rendering() {
        let tabs = vec![("Tab 1", false), ("Tab 2", true)];
        let bar = render_tab_bar(&tabs);
        assert!(bar.contains("Tab 1"));
        assert!(bar.contains("Tab 2"));
    }

    #[test]
    fn hex_to_ansi_conversion() {
        assert!(hex_to_ansi256("#ff0000").is_some());
        assert!(hex_to_ansi256("#00ff00").is_some());
        assert!(hex_to_ansi256("#0000ff").is_some());
        assert!(hex_to_ansi256("invalid").is_none());
        assert!(hex_to_ansi256("transparent").is_none());
    }

    #[test]
    fn truncate_strings() {
        assert_eq!(truncate("short", 10), "short");
        assert_eq!(truncate("a very long string", 10), "a very...");
    }
}
