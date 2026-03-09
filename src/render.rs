//! GPU rendering module -- wgpu pipeline for browser content.
//!
//! Uses garasu for GPU context, egaku for browser chrome (address bar,
//! tabs, bookmarks bar), and fude for rich text content rendering.
//! Renders: text blocks, inline images, form controls, scroll regions.
//!
//! # Architecture
//!
//! The rendering pipeline is structured as follows:
//!
//! ```text
//! LayoutTree (positioned boxes)
//!       |
//!       v
//! Render commands (garasu primitives)
//!   - Text spans (fude rich text with computed styles)
//!   - Image textures (decoded via image crate)
//!   - Background rectangles (solid colour fills)
//!   - Borders (line primitives)
//!       |
//!       v
//! Browser chrome (egaku widgets)
//!   - Tab bar: open tabs, active tab highlight
//!   - Address bar: current URL, loading indicator
//!   - Bookmarks bar: starred pages
//!   - Scroll bars: vertical/horizontal
//!       |
//!       v
//! garasu GPU context
//!   - wgpu render pass
//!   - Swapchain present
//! ```
//!
//! # Planned Components
//!
//! - **Page content renderer**: Walks the layout tree and emits garasu draw
//!   commands for each visible box. Text is rendered via fude with styles
//!   from the CSS cascade. Background colours and borders are drawn as
//!   garasu rectangle primitives.
//!
//! - **Image renderer**: Inline images (PNG, JPEG, GIF, WebP) are decoded
//!   via the `image` crate and uploaded as GPU textures. The layout box
//!   dimensions determine the display size.
//!
//! - **Browser chrome**: The tab bar, address bar, bookmarks bar, and status
//!   bar are rendered using egaku widget primitives. These are composited
//!   on top of the page content.
//!
//! - **Scroll manager**: Tracks the scroll offset for the page content
//!   viewport. Renders scroll indicators. Handles keyboard (j/k, Page Up/Down)
//!   and mouse wheel input.
//!
//! - **Link highlighting**: Hovering or focusing a link highlights it with
//!   the configured `link_color`. Clicking navigates to the href.
//!
//! - **Form controls**: Basic rendering of `<input>`, `<textarea>`,
//!   `<select>`, and `<button>` elements using egaku widgets.
//!
//! # Dependencies
//!
//! - `garasu`: GPU context management, texture loading, shader pipeline
//! - `egaku`: Widget toolkit for browser chrome (tabs, address bar, scroll)
//! - `fude`: Rich text rendering for HTML content
//! - `wgpu`: Low-level GPU API (managed by garasu)
//! - `winit`: Window creation and event loop (managed by garasu)
//! - `image`: Decoding inline images (PNG, JPEG, GIF, WebP)
