# md

`md` is a Markdown reader with two fronts in one repo:

- a full-screen terminal UI for Linux and macOS (binary name `md`)
- a browser viewer at [https://md-tui.over9000.cc](https://md-tui.over9000.cc)

Both typeset CommonMark and the usual GitHub extras (tables, task lists, strikethrough, fenced code) so a README is faster to scan than the raw file.

```text
crates/md/     Rust TUI. `cargo test` and `cargo run -p md` from the repo root.
fixtures/      Shared samples. The site serves these as static files.
web/           Browser viewer (Vite + TypeScript).
wrangler.jsonc Cloudflare Worker static assets. Not Pages.
```

## Terminal

Install a prebuilt binary (macOS arm64/x86_64, Linux x86_64/aarch64, WSL) into `~/.local/bin`:

```bash
curl -fsSL https://md-tui.over9000.cc/install.sh | bash
```

Set `MD_INSTALL_DIR` to pick another directory or `MD_VERSION=v0.1.0` to pin a release. Releases come from pushing a `v*` tag (`.github/workflows/release.yml`).

From source, with Rust 1.83 or newer (edition 2021), on Linux or macOS. The repo pins 1.83 in `rust-toolchain.toml`.

```bash
cargo install --path crates/md
md README.md
```

From a checkout, without installing:

```bash
cargo run --release -p md -- README.md
```

### Usage

```bash
md README.md
md --theme paper NOTES.md
md --theme terminal README.md
cat README.md | md
md - < README.md
md --print README.md | less -R
```

| Flag | Meaning |
| --- | --- |
| `-t, --theme ink\|paper\|terminal` | `ink` is the dark default. `paper` is light. `terminal` uses the host palette and does not paint a background. |
| `-p, --print` | Write styled text to stdout and exit. Links are OSC 8 hyperlinks. |
| `-w, --width N` | Wrap width for `--print`. Defaults to the terminal width, or 80. |

If stdout is not a terminal, `md` prints and exits instead of opening the full-screen UI. A missing or unreadable file is reported on stderr and the process exits `1`. No file and a terminal stdin exits `2`.

```bash
md fixtures/showcase.md
```

### Keys

| Key | Action |
| --- | --- |
| `j` `k` or `↓` `↑` | Scroll one line |
| `d` `u` or Ctrl-d / Ctrl-u | Half page |
| Space, PgDn, Ctrl-f | Page down |
| `b`, PgUp | Page up |
| `gg`, Home | Top |
| `G`, End | Bottom |
| `h` `l` or `←` `→` | Scroll sideways (useful in code blocks) |
| `0` | Reset sideways scroll |
| Mouse wheel | Scroll |
| `/` | Search. Type the query, Enter to jump to the first hit. Esc cancels. |
| `n` `N` | Next / previous match. A lowercase query ignores ASCII case. |
| `[` `]` | Previous / next heading |
| `o` | Toggle the heading outline. Click a heading to jump. |
| Tab / Shift-Tab | Focus the next / previous link |
| Enter | Open the focused link |
| `f` | Link picker. `j`/`k` move, Enter opens, `1`–`9` open that entry, Esc closes. |
| Click | Open the link under the pointer. Hover highlights it and shows the URL. |
| Backspace, Alt-Left | Back to the previous file |
| `t` | Cycle theme: ink, paper, terminal |
| `?` | Key help. Esc closes it. |
| `q`, Esc, Ctrl-c | Quit. Esc closes help, search, or the link picker first. |

Links:

- `http`, `https`, and `mailto` open in the system handler (`xdg-open` on Linux, `open` on macOS).
- Relative `.md` / `.markdown` links open in the reader. `#anchors` scroll to a heading.
- Other existing local files are handed to the system opener.

The same URLs are also emitted as OSC 8 hyperlinks, so terminals that understand them (iTerm2, kitty, WezTerm, Ghostty, and others) can treat the span as a link. In the full-screen UI, mouse capture handles the click itself. `--print` leaves the hyperlinks for the terminal.

### What a page looks like

Opening `fixtures/showcase.md` fills the screen with the terminal's alternate buffer:

```text
 Showcase
 ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

 A fixture for md, a terminal Markdown reader.

 Why this exists
 ───────────────────────────────────────────────

 What you can do
 ───────────────

 • Scroll with j / k, the arrow keys, or the mouse wheel
 • Open Ratatui↗ in a browser

 │ rust
  1 │ fn main() {
  2 │     let name = "md";
  3 │ }

 Feature     Keys          Notes
 ──────────  ────────────  ────────────────
 Scroll      j k           mouse wheel too

 showcase.md  12%  Why this exists     j/k scroll  / find  tab link  q quit
```

Heading size follows the VS Code preview scale: a heavy rule under `#`, a thin full-width rule under `##`, a short rule under `###`, and no rule from `####` down. Code sits on a rail with a language name, links are underlined, and the status line keeps the file name visible beside the key hints. `q` leaves the alternate screen and restores the cursor.

### Architecture

1. **Parse.** `pulldown-cmark` events become an owned block tree: headings, paragraphs, lists, quotes, tables, code, and links.
2. **Layout.** Blocks wrap to the current width into styled lines. Heading slugs and link ids are recorded with the line they land on. Fenced code is highlighted with a small built-in lexer.
3. **Render.** ratatui draws the visible window, outline, search bar, and status line. After each frame, link spans are painted again with OSC 8.
4. **Input.** crossterm keys and mouse update scroll, search, focus, and history. HTTP(S) links launch the system opener. Local Markdown replaces the buffer and pushes the previous file so Backspace can return.

Resize rebuilds the layout. The theme is applied at draw time, so `t` does not rewrap the text.

## Web

The site is the same reader in a browser window: ink (and paper) colors, heading rules, a code rail with highlighting, tables, task lists, quotes, alerts, and underlined links. Open [https://md-tui.over9000.cc](https://md-tui.over9000.cc), or run it locally:

```bash
pnpm install
pnpm dev
```

Then open the printed localhost URL. The dev server serves `fixtures/` so the first page is `fixtures/showcase.md`.

| Action | How |
| --- | --- |
| Open a file | Open button, or drop a `.md` file anywhere on the page |
| Paste | Paste Markdown text, or a copied file |
| Load a URL | `?url=https://example.com/README.md` (the host must allow browser fetches) |
| Outline | Left rail, or `o`. Click a heading to jump. |
| Search | `/`, then Enter. `n` / `N` move. A lowercase query ignores ASCII case. |
| Move | `j` `k`, `gg` top, `G` bottom, `d` `u`, Space, `b`, `[` `]` |
| Theme | `t`, or the theme button. `ink` and `paper`. |
| Help | `?` |

Relative `.md` links work for documents that were fetched (the bundled showcase, or `?url=`). A pasted buffer has no directory, so those links stay on the page and the status line says so. Backspace returns to the previous document.

### Why TypeScript, not WASM

The terminal crate's interesting work is cell layout: unicode display width, a gutter, and ratatui spans. That grid is the wrong artifact for a browser. The page wants real HTML so text is selectable, headings are headings, links are links, and font sizes can follow the VS Code scale the terminal can only approximate with weight and rules.

Sharing the parser would still leave an HTML emitter, a theme, and the keyboard UI to write in TypeScript. `marked` (GFM) plus `highlight.js` covers the same CommonMark and GitHub surface, and the heading slugs use the same rules as `slugify` in `crates/md/src/open.rs`. A WASM build of pulldown-cmark would add a second toolchain and a binary blob without changing how the page looks. The colors in `web/src/style.css` are the ink and paper RGB values from `crates/md/src/theme.rs`.

## Deploy

Production is a Cloudflare Worker named `md-tui` on account `2e169e038d9f430944291e98aa79e684`, serving the Vite build as static assets. It is not Cloudflare Pages.

`wrangler.jsonc` sets `assets.directory` to `./web/dist` and `not_found_handling` to `single-page-application`, and attaches the custom domain `md-tui.over9000.cc`.

```bash
pnpm install
pnpm test
pnpm build
pnpm exec wrangler deploy
```

GitHub Actions:

- `.github/workflows/ci.yml` runs `cargo test` on Linux and macOS, and runs the web tests plus a production build, on pull requests and on `main`.
- `.github/workflows/deploy.yml` runs on push to `main` and `workflow_dispatch`. It refuses to continue if `CLOUDFLARE_API_TOKEN` or `CLOUDFLARE_ACCOUNT_ID` is missing, then installs, tests, builds, deploys, and curls `https://md-tui.over9000.cc/` until the response is HTTP 200 and the body contains `G-FG52JBJRPS`.
- `.github/workflows/release.yml` builds `md` release binaries for Linux x86_64 and macOS (arm64 and x86_64) when a `v*` tag is pushed.

The page `<head>` includes GA4 measurement id `G-FG52JBJRPS`.

## Development

```bash
cargo test
cargo build --release -p md
cargo run -p md -- fixtures/showcase.md
cargo run -p md -- --print fixtures/showcase.md

pnpm install
pnpm test
pnpm build
```

See `TEST_PLAN.md` for a manual pass of the terminal UI.

## License

MIT. See [LICENSE](LICENSE).
