# Showcase

A fixture for **md**, a terminal Markdown reader. Open it with:

```bash
md fixtures/showcase.md
```

Headings, lists, tables, quotes, and fenced code should read as a typeset page. The raw `#` and fences stay in the file, not on screen.

## Why this exists

Reading docs in a terminal is fine until the source gets in the way. This view keeps the words and gives the structure a little room: color, weight, and a quiet code rail.

### What you can do

- Scroll with `j` / `k`, the arrow keys, or the mouse wheel
- Search with `/`, then `n` and `N`
- [Jump to the table](#tables)
- Open [Ratatui](https://ratatui.rs) in a browser
- Follow [field notes](notes.md) inside the app, then come back with Backspace

## Lists and tasks

1. First ordered item, long enough to wrap onto a second line when the terminal is a bit narrow.
2. Second item, with `inline code` in the sentence.

- unordered item
  - nested bullet
- a [link inside a list](https://www.rust-lang.org)

- [x] typeset the common blocks
- [ ] keep the dependency list small

## Quotes

> Markdown is a lightweight markup language for writing structured text that still looks like text.

> [!NOTE]
> Alerts keep a short label and a bar, so a warning is obvious without a raw `[!NOTE]` tag.

> [!WARNING]
> Resize the terminal before you judge wrapping. The layout is rebuilt to the new width.

## Code

Run the reader on this file with `md fixtures/showcase.md`.

```rust
fn main() {
    // greet the reader
    let name = "md";
    println!("hello, {name}");
}
```

```python
def greet(name: str) -> str:
    return f"hello, {name}"
```

A deliberately wide line, for `h` and `l`:

```text
alpha bravo charlie delta echo foxtrot golf hotel india juliet kilo lima mike november oscar papa quebec romeo sierra tango
```

## Tables

| Feature | Keys | Notes |
| --- | --- | --- |
| Scroll | `j` `k` | mouse wheel too |
| Search | `/` `n` `N` | lowercase query folds ASCII case |
| Links | click, Tab, Enter | http(s) and local `.md` |
| Outline | `o` | heading list on the left |

| Name | Role | Year |
| :--- | :---: | ---: |
| Ada Lovelace | Mathematician | 1843 |
| Grace Hopper | Admiral | 1952 |

## Emphasis

This is ~~not raw source~~ a rendered paragraph, with *emphasis* and **weight**.

## A missing link

[This file is not in the tree](missing.md) should report an error in the status line and leave you on this page.

[Back to the top](#showcase)
