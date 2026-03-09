# Nami (波)

GPU-rendered TUI browser. Full web rendering in a GPU-accelerated terminal interface.

## Features

- HTML5 parsing via html5ever
- CSS parsing and cascade via lightningcss
- Flexbox/grid layout via taffy
- GPU text and image rendering via garasu
- Browser chrome (tabs, address bar, bookmarks) via egaku
- Rich text content rendering via fude
- HTTPS-only mode, tracker blocking
- Keyboard-driven navigation
- Hot-reloadable configuration via shikumi

## Architecture

| Module | Purpose |
|--------|---------|
| `dom` | html5ever DOM tree with traversal/query |
| `css` | lightningcss parsing, cascade, computed styles |
| `layout` | taffy flexbox/grid layout engine |
| `fetch` | reqwest HTTP client (cookies, compression, redirects) |
| `render` | GPU page rendering via garasu + fude |
| `config` | shikumi-based configuration |

## Rendering Pipeline

```
URL → fetch (reqwest) → HTML (html5ever) → DOM tree
                           ↓
CSS (lightningcss) → computed styles → layout (taffy) → render (garasu)
```

## Shared Libraries

- **garasu** — GPU rendering engine
- **egaku** — browser chrome widgets (tabs, address bar, bookmarks)
- **fude** — rich text rendering (HTML content → styled spans)
- **tsunagu** — daemon IPC (background prefetch)
- **shikumi** — config discovery + hot-reload

## Build

```bash
cargo build
cargo run -- "https://example.com"
cargo run -- source "https://example.com"
cargo run -- text "https://example.com"
```

## Configuration

`~/.config/nami/nami.yaml`

```yaml
homepage: "about:blank"
appearance:
  font_size: 14.0
  show_images: true
  link_color: "#5e81ac"
network:
  timeout_secs: 30
  follow_redirects: true
privacy:
  block_trackers: true
  https_only: true
```
