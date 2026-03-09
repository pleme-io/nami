# Aranami (荒波) — GPU-Rendered TUI Browser

A GPU-accelerated TUI browser that renders real HTML+CSS in a terminal-like
interface. Pure Rust with own rendering engine (no embedded Chromium/Firefox).
Uses nami-core for the content pipeline (DOM, CSS, layout) and garasu for
GPU rendering.

Binary name: `nami`. Crate name: `aranami` (nami was taken on crates.io).

## Build & Test

```bash
cargo build
cargo run -- "https://example.com"
cargo test --lib
nix build                     # Nix package
nix run .#rebuild             # rebuild HM module (from nix repo)
```

## Competitive Position

| vs | Aranami advantage |
|----|-------------------|
| **Browsh** | Pure Rust with own rendering (no Firefox dependency), smaller footprint, Rhai scripting, MCP |
| **Carbonyl** | No Chromium dependency, nami-core for DOM/CSS/layout, plugin ecosystem, Nix-native |
| **w3m** | GPU-rendered text (not ncurses), CSS cascade support, modern HTML5, plugins, MCP |
| **Lynx** | GPU rendering, CSS support, scriptable, MCP-drivable, image support |
| **Links2** | Full CSS flexbox/grid via taffy, Rhai scripting, MCP server, Nix integration |

## Architecture

### Content Pipeline

```
URL --> todoku (HTTP fetch) --> HTML --> nami-core DOM (html5ever)
                                              |
                                   nami-core CSS (lightningcss)
                                              |
                                   nami-core Layout (taffy)
                                              |
                               LayoutTree (positioned boxes)
                                              |
                        Render commands (garasu GPU primitives)
                          - Text spans (mojiban rich text)
                          - Image textures (decoded via image crate)
                          - Background rectangles (solid fills)
                          - Borders (line primitives)
                                              |
                        Browser chrome (egaku widgets)
                          - Tab bar, address bar, bookmarks bar
                          - Scroll indicators
                          - Status bar (URL, loading, blocked count)
                                              |
                        garasu GPU context --> wgpu render pass --> present
```

### Source Modules

| Module | Lines | Purpose |
|--------|-------|---------|
| `dom.rs` | ~560 | html5ever DOM tree, node traversal, element queries |
| `css.rs` | ~580 | lightningcss parsing, specificity cascade, computed styles |
| `layout.rs` | ~450 | taffy flexbox/grid to absolute positions and sizes |
| `fetch.rs` | ~220 | reqwest HTTP client: cookies, gzip/brotli, redirects, proxy |
| `config.rs` | ~120 | shikumi ConfigStore with hot-reload |
| `render.rs` | ~70 | GPU content rendering scaffold (garasu + mojiban) |
| `main.rs` | ~100 | CLI entry point, URL argument, event loop setup |

**Note**: The dom/css/layout modules currently duplicate nami-core functionality.
These should be migrated to use nami-core as a dependency once the library is
stabilized. The duplication exists because nami predates nami-core.

### Planned migration to nami-core

The `dom.rs`, `css.rs`, and `layout.rs` modules in this repo should be replaced
by `nami-core` imports. After migration, this repo retains only:
- `main.rs` -- CLI and app entry point
- `render.rs` -- GPU rendering (nami-core is rendering-agnostic)
- `config.rs` -- aranami-specific config (extends nami-core config)
- `keybind.rs` (new) -- vim-style keyboard navigation
- `chrome.rs` (new) -- browser chrome widgets (address bar, tab bar, status bar)

### Dependency note

The old library names (`fude`, `hikidashi`, `kotoba`) have been updated to
the current names (`mojiban`, `hasami`, `kaname`).

---

## Shared Library Integration

| Library | Used For |
|---------|----------|
| **nami-core** | DOM parsing, CSS cascade, layout computation, content blocking, bookmarks, history |
| **garasu** | GPU context, text rendering, image textures, shader pipeline |
| **madori** | App framework (event loop, render loop, input dispatch) |
| **egaku** | Browser chrome widgets (address bar, tab bar, bookmarks bar, scroll) |
| **irodzuki** | GPU theming (base16 to wgpu uniforms for page rendering) |
| **mojiban** | HTML text to styled spans for GPU rendering |
| **todoku** | HTTP fetch (cookies, gzip/brotli, redirects, proxy, HTTPS) |
| **hasami** | Clipboard (copy page text, URL) |
| **tsunagu** | Background prefetch daemon mode |
| **shikumi** | Config discovery + hot-reload |
| **kaname** | Embedded MCP server |
| **soushi** | Rhai scripting engine for user plugins |
| **awase** | Vim-style keyboard navigation system |
| **tsuuchi** | Notifications (download complete, blocked content alert) |

---

## Configuration

- **File**: `~/.config/nami/nami.yaml`
- **Env override**: `NAMI_CONFIG=/path/to/config.yaml`
- **Env prefix**: `NAMI_` (e.g., `NAMI_HOMEPAGE=https://example.com`)
- **Hot-reload**: shikumi ArcSwap + file watcher
- **HM module**: `blackmatter.components.nami.*`

Config sections:
```yaml
homepage: "about:blank"
search_engine: "https://www.google.com/search?q=%s"
content_blocking:
  enabled: true
  block_trackers: true
  block_ads: false
  filter_lists: []            # paths to EasyList-format files
privacy:
  https_only: false
  do_not_track: true
  clear_on_exit: false
appearance:
  font_size: 14
  images: true                # inline image rendering
  dark_mode: true
  link_color: "#88c0d0"       # Nord frost
keybindings: {}               # override defaults
```

---

## GPU Rendering

### Page Content Rendering

The render pipeline walks the nami-core `LayoutTree` and emits garasu draw commands:

1. **Background pass**: Draw background rectangles for each layout box with
   computed `background-color`
2. **Border pass**: Draw border lines with computed widths and colors
3. **Text pass**: For each text node, convert to mojiban styled spans using
   computed CSS properties (color, font-size, font-weight, font-style),
   then render via garasu text renderer
4. **Image pass**: Decode inline images (PNG, JPEG, GIF, WebP) via `image` crate,
   upload as GPU textures, render at layout-computed dimensions
5. **Chrome pass**: Render browser chrome (egaku widgets) on top of page content
6. **Post-processing**: Optional WGSL shader chain from `~/.config/nami/shaders/`

### Browser Chrome (egaku widgets)

- **Tab bar**: Open tabs with titles, active tab highlight, close buttons
- **Address bar**: Current URL (editable), loading indicator, bookmarked icon
- **Bookmarks bar**: Quick-access bookmarks (optional, togglable)
- **Status bar**: Hovered link URL, blocked request count, loading progress
- **Scroll indicators**: Vertical/horizontal scroll position

---

## Keyboard Navigation (awase)

Vim-style modal navigation:

| Mode | Purpose | Enter via |
|------|---------|-----------|
| **Normal** | Page navigation, link following | Default, `Esc` |
| **Insert** | Text input (address bar, forms) | `i`, `o` (open URL), click in input |
| **Command** | `:` prefix commands | `:` |
| **Search** | `/` forward search, `?` backward | `/` or `?` |
| **Follow** | Link hint labels | `f` |

**Normal mode bindings**:
| Key | Action |
|-----|--------|
| `j`/`k` | Scroll down/up |
| `d`/`u` | Half-page down/up |
| `gg`/`G` | Top/bottom of page |
| `f` | Follow link (show hint labels) |
| `F` | Follow link in new tab |
| `o` | Open URL (enter Insert mode in address bar) |
| `O` | Open URL in new tab |
| `H`/`L` | Go back/forward in history |
| `r` | Reload page |
| `gt`/`gT` | Next/previous tab |
| `t` | New tab |
| `x` | Close current tab |
| `yy` | Copy current URL to clipboard |
| `p` | Open URL from clipboard |
| `/` | Search forward on page |
| `n`/`N` | Next/previous search match |
| `:` | Command mode |

**Command mode**:
| Command | Action |
|---------|--------|
| `:open <url>` | Navigate to URL |
| `:tabopen <url>` | Open URL in new tab |
| `:bookmark [tags]` | Bookmark current page |
| `:bookmarks` | Show bookmark list |
| `:history` | Show browsing history |
| `:set <key> <value>` | Change config at runtime |
| `:js-toggle` | Toggle JavaScript (when JS engine added) |
| `:images-toggle` | Toggle image rendering |

---

## MCP Server (kaname)

Embedded MCP server via stdio transport, discoverable at `~/.config/nami/mcp.json`.

**Standard tools**: `status`, `config_get`, `config_set`, `version`

**Browser-specific tools**:
| Tool | Description |
|------|-------------|
| `navigate` | Navigate to a URL |
| `get_dom` | Get DOM tree (or subtree via selector) as JSON |
| `get_links` | Get all links on the current page |
| `click_link` | Click a link by index or selector |
| `search_text` | Search for text on the current page |
| `get_page_source` | Get raw HTML source |
| `bookmark_add` | Add current page to bookmarks |
| `history_search` | Search browsing history |
| `screenshot` | Capture current viewport as PNG |
| `extract_text` | Extract text content from a CSS selector |

---

## Plugin System (soushi + Rhai)

Scripts loaded from `~/.config/nami/scripts/*.rhai`.

**Rhai API**:
```
nami.goto(url)               // navigate to URL
nami.back()                  // go back in history
nami.forward()               // go forward in history
nami.links()                 // get all links on page as array
nami.follow(n)               // follow link by index
nami.search(text)            // search for text on page
nami.bookmark(url, tags)     // add bookmark with tags
nami.extract_text(selector)  // get text content of CSS selector
nami.dom_query(selector)     // query DOM elements
nami.tab_new()               // open new tab
nami.tab_close()             // close current tab
nami.scroll_to(selector)     // scroll element into view
nami.set_theme(name)         // switch theme
nami.block_domain(domain)    // add domain to content blocker
```

**Event hooks**: `on_page_load`, `on_navigate`, `on_link_hover`,
`on_download_start`, `on_download_complete`, `on_blocked_request`

**Use cases**: Custom start pages, reading mode transforms, auto-bookmarking,
content extraction pipelines, automated browsing workflows.

---

## Content Blocking (nami-core)

EasyList-compatible filter engine from nami-core:
- Domain blocking with subdomain matching (`||domain.com^`)
- Exception rules (`@@||domain.com^`)
- Pattern-based URL blocking (substring match)
- Resource type filtering (script, image, stylesheet, etc.)
- Per-session statistics (checked/blocked/allowed counts)
- Filter lists loaded from config or `~/.config/nami/filters/`

---

## Roadmap

### Phase 1 -- Core Pipeline [DONE]
HTML parsing (html5ever), CSS cascade (lightningcss), layout (taffy),
HTTP fetch (reqwest), basic GPU rendering scaffold.

### Phase 2 -- Migrate to nami-core [NEXT]
Replace duplicated dom/css/layout modules with nami-core dependency.
Wire in todoku for HTTP instead of direct reqwest. Add content blocking.

### Phase 3 -- Rendering
Full GPU rendering pipeline: text via mojiban, images via image crate,
backgrounds and borders via garasu rectangles. Browser chrome via egaku.

### Phase 4 -- Navigation
Vim-style keyboard navigation via awase. Link following with hint labels.
Address bar editing. Tab management.

### Phase 5 -- Features
Bookmarks and history (nami-core storage). Content blocking UI.
Search on page. Form input support (basic).

### Phase 6 -- Programmability
MCP server (kaname). Rhai scripting (soushi). Plugin loading.
Daemon mode (tsunagu) for background page monitoring.

### Phase 7 -- Polish
JavaScript engine (boa_engine, optional). Performance optimization.
Accessibility. HTTPS-only mode. Cookie management UI.

---

## Design Decisions

### Why nami-core (not inline implementation)?
The DOM, CSS, and layout code is shared between aranami (TUI browser) and
namimado (desktop browser). Extracting it into nami-core avoids duplication
and ensures both browsers behave identically for content rendering.

### Why html5ever + lightningcss + taffy (not a full browser engine)?
These three crates provide spec-compliant HTML parsing, fast CSS handling,
and correct flexbox/grid layout without pulling in a full browser engine.
The combination is lightweight (~5MB binary) and fully Rust-native.

### Why no JavaScript initially?
JavaScript execution is the most complex part of a browser engine. Starting
with static HTML+CSS rendering lets us get the content pipeline correct first.
boa_engine (pure Rust JS engine) can be added later behind a feature flag.

### Why GPU rendering (not terminal escape sequences)?
Terminal-based renderers are limited by the terminal's capabilities (colors,
character set, cursor positioning). GPU rendering via garasu gives us full
control over text styling, inline images, and smooth scrolling.

### Why todoku (not direct reqwest)?
todoku provides authenticated HTTP with retry, shared across all pleme-io apps.
Using it ensures consistent behavior (timeouts, TLS config, proxy handling)
and avoids duplicating HTTP client configuration in every app.

---

## Nix Integration

- **Flake**: `packages.aarch64-darwin.default`, `overlays.default`, `homeManagerModules.default`
- **HM module path**: `module/default.nix` (to be created, currently scaffold)
- **Build**: `pkgs.rustPlatform.buildRustPackage` (migrate to substrate `rust-tool-release-flake.nix`)
- **Config management**: HM module will generate `~/.config/nami/nami.yaml` from typed Nix options
- **Filter lists**: HM module can deploy EasyList files to `~/.config/nami/filters/`
