# rechner

A small desktop calculator in Rust and GTK4 that handles both common ways of
writing numbers, and lets you switch between them at any time:

| Notation | Example |
| --- | --- |
| Decimal comma | `1.234,5` |
| Decimal point | `1,234.5` |

## Typing separators: `..` becomes `,`

Whichever notation you use, you can type every number using only the period
key:

> **Type `.` twice in a row and the two periods become a single `,`.**

| Notation | You type | You get |
| --- | --- | --- |
| Decimal comma | `1..5` | `1,5` |
| Decimal comma | `1.234..5` | `1.234,5` |
| Decimal point | `1..234.5` | `1,234.5` |

The rule is the same in both notations, and it is safe because two periods in a
row are never valid input:

- **Decimal comma** (`1.234,5`): `.` is the group separator and `..` gives the
  decimal comma.
- **Decimal point** (`1,234.5`): `.` is the decimal point and `..` gives the
  group separator.

The replacement happens only when the second `.` directly follows the first,
and Ctrl+Z undoes it.

The **numpad decimal key** works the other way round. Press it once for the
decimal separator, and press it twice for the group separator.

Grouping is validated in both cases, so `1.23` (decimal comma) is reported as
an error instead of being read as `123`.

## Features

- **Notation:** choose decimal comma or decimal point in the *Notation* menu.
  An expression you have already typed is converted along with it.
- **Result line:** the result shows as a live preview while you type and is
  confirmed with Enter or `=`. Click it, or press Ctrl+C, to copy the result
  without group separators.
- **Exact arithmetic:** uses [`rust_decimal`](https://crates.io/crates/rust_decimal),
  so `0.1 + 0.2` is `0.3`.
- **Operators:** `+ − × ÷`, parentheses, `x²` (Shift+click for `x^y`), `√` and `%`.
  `200 + 10%` is `220`, `2(3+4)` is `14`, and a missing `)` at the end is closed
  for you.
- **Languages:** English (default) and German, switched in the *Language* menu.
- **Themes:** Light and Dark, plus Nord, Solarized Dark and Light, Dracula,
  Gruvbox Dark and Light, Catppuccin Mocha and Latte, Tokyo Night and One Dark
  (*View → Theme*).
- **Focus:** the window turns translucent when it loses focus. Choose 20%,
  50%, 80% or 100% (opaque) under *View → Opacity when unfocused*.
- **Single instance:** launching it again raises the open window.

Notation, theme, language and opacity are saved to `~/.config/rechner/config`.

## Keys

| Key | Action |
| --- | --- |
| `.` `.` | Turn the two periods into `,` |
| Numpad decimal | Decimal separator; twice for the group separator |
| Enter, `=` | Evaluate |
| Esc | Clear; on an empty field, close |
| Ctrl+Z | Undo |
| Ctrl+C | Copy result (when nothing is selected) |
| Ctrl+Q | Quit |

After `=`, an operator continues from the result and a digit starts over.

## Build and install

Requires GTK ≥ 4.12 and a Rust toolchain.

```sh
cargo install --path .
```

The binary is `rechner` and the app ID is `io.github.HollowPhobos.rechner`.

### sway

```
bindsym XF86Calculator exec ~/.cargo/bin/rechner
for_window [app_id="io.github.HollowPhobos.rechner"] floating enable
```

## Tests

The parser and formatter in `src/calc.rs` have no GTK dependency:

```sh
cargo test
```
