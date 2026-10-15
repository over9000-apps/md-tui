# Test plan

Binary name: `md`. The crate lives in `crates/md`. Build from the repo root:

```bash
cargo test
cargo build --release -p md
./target/release/md fixtures/showcase.md
```

Fixture: `fixtures/showcase.md` (relative link target: `fixtures/notes.md`).

## Open and render

1. `./target/release/md fixtures/showcase.md` opens a full-screen view immediately.
2. The first heading reads `Showcase` with a heavy full-width rule under it. The raw line `# Showcase` is not shown. `##` headings have a thin full-width rule. `###` headings have a short rule only as wide as the title. `####` and below have no rule, and lower levels get quieter.
3. Fenced code shows `fn main()` (or the python `def greet`) on a distinct background or rail. The fence markers ` ``` ` are not shown. A language label (`rust` or `python`) is visible.
4. The table under `Tables` has a header row (`Feature`, `Keys`, `Notes`) and a horizontal rule. Columns line up.
5. Task items show a check mark and an open marker, not `- [x]` / `- [ ]`.
6. `Ratatui` is underlined (and has a small arrow). It does not show the raw `](https://ratatui.rs)` syntax.
7. Resize the terminal. Text reflows. The status line stays on the last row. No panic.

## Scroll, search, headings

8. `j` / `k` and the arrow keys move one line. Page Down / Space and Page Up / `b` move about a page. Ctrl-d and Ctrl-u move half a page.
9. `gg` or Home goes to the top. `G` or End goes to the bottom.
10. Mouse wheel scrolls.
11. `h` and `l` scroll sideways on the long `text` code line. `0` returns to the left edge.
12. `/table` then Enter jumps to a match. `n` and `N` move between matches. Esc during typing cancels the search.
13. `]` and `[` move to the next and previous heading.
14. `o` opens an outline. Clicking a heading jumps. `o` closes it.

## Links and mouse

15. Hover a link: the span highlights and the URL appears in the status line.
16. Click `https://ratatui.rs` (the Ratatui link). The system browser or `xdg-open` is invoked. The UI stays up. If no opener is installed, the status line shows the error.
17. Click `field notes` (`notes.md`). The view switches to Field notes. Backspace returns to the showcase, near the previous scroll position.
18. Click `Jump to the table` (`#tables`). The view scrolls to the Tables heading.
19. Tab and Shift-Tab move focus between links (focused link is highlighted). Enter opens the focused link.
20. `f` opens the link picker. `j`/`k` move, Enter opens, `1` opens the first link, Esc closes the picker without quitting.
21. Click `This file is not in the tree`. The status line reports that it cannot be read. The showcase stays open.

## Themes, stdin, errors, quit

22. `t` cycles ink → paper → terminal. `--theme paper` starts in the light theme. `--theme terminal` does not force a dark background.
23. `md --print fixtures/showcase.md` prints styled text and exits. The output contains `Showcase` and an OSC 8 sequence (`ESC ] 8 ;;`) for `https://ratatui.rs`. It does not contain a raw `# Showcase` line.
24. `cat fixtures/showcase.md | ./target/release/md` opens the UI titled as stdin. `md --print < fixtures/notes.md` prints and exits.
25. `./target/release/md does-not-exist.md` prints `md: cannot read ...` to stderr and exits non-zero. The terminal is not left in raw mode.
26. `q` quits. Run `echo ok` afterward: the shell prompt is normal, characters echo, and the cursor is visible. Repeat with Esc from the main view (not from search, help, or the picker) and with Ctrl-c.
27. `?` shows the key list. Esc closes it. `q` from the help screen quits.

## Automated

`cargo test` covers parsing, wrapping, search, relative-link history, OSC 8 in `--print`, and a headless render that checks headings, code, and link hit testing.
