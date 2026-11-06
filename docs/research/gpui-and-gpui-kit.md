# What GPUI and gpui-kit offer for this app

- Status: findings
- Date: 2026-11-06
- Question: [#3](https://github.com/brzzdev/uk-bank-holidays/issues/3)

## Context

The app is a small personal macOS window that lists UK bank holidays by year and month, highlights
four-day weekends and predicted bank holidays, and switches division. A CLI shares the core. The
crate is edition 2024, `rust-version = "1.99"`, clippy pedantic. Distribution and signing are
post-v1 but must stay possible.

The question: which widgets, theming, window chrome and menu support GPUI and gpui-kit provide; how
an app depends on them; how mature and stable they are; and whether anything in them gets in the way
of later signing and distribution.

## Findings

### What each project is

- **GPUI** is Zed's GPU-accelerated UI framework. It is a hybrid immediate/retained-mode framework
  with a Tailwind-style `div()` builder, `Entity` state, `Render`/`RenderOnce` views, actions and
  key bindings. Its README says it is "still pre-1.0. There will often be breaking changes between
  versions" and asks for the latest stable Rust
  ([zed `crates/gpui/README.md`](https://github.com/zed-industries/zed/blob/main/crates/gpui/README.md)).
  Docs are thin: the README, the crate root, `docs/contexts.md`, `docs/key_dispatch.md`, examples, and
  Zed's own code ([gpui.rs](https://gpui.rs/)). GPUI itself has no widget library. Zed's `ui` crate
  is Zed-internal.
- **gpui-kit** (Longbridge, Apache-2.0) is the old `gpui-component` under a new name and an umbrella
  crate. Its one `gpui-kit` dependency bundles GPUI, `gpui-base` (unstyled behaviour), `gpui-component`
  (styled, shadcn-like components) and default icon assets. It claims "75+ components" and "used to
  build Longbridge Pro from day one"
  ([gpui-kit.com](https://gpui-kit.com/), [README](https://github.com/longbridge/gpui-kit/blob/main/README.md)).

### Widgets relevant to this app

These come from `crates/component/src/` in
[longbridge/gpui-kit](https://github.com/longbridge/gpui-kit/tree/main/crates/component/src):

| Need | What exists | Fit |
| --- | --- | --- |
| Holiday list | `list/` (`List` + delegate), `virtual_list.rs`, `table/`, `description_list.rs`, `group_box.rs` | Good. Plain `div` + `v_flex` is enough for at most ~30 rows a year. |
| Year/month calendar | `time/calendar.rs` (styled) over `gpui-base`'s `calendar.rs`; `CalendarState` with `number_of_months`, `year_range`, `disabled_matcher`, `Matcher::custom(Fn(&NaiveDate) -> bool)` | **Partial.** See below. |
| Division switcher / year switcher | `tab/` with `TabVariant::Segmented` (also `Pill`, `Underline`, …); `button/button_group.rs`; `button/toggle.rs`; `select/` | Good. |
| Markers for "four-day weekend", "predicted" | `tag.rs` (`Tag::primary/success/warning/info/custom`, `outline`), `badge.rs` (count/dot/icon on an element), `tooltip.rs`, `hover_card.rs` | Good. |
| Grid layout | `grid_layout.rs`, and GPUI's own flex/grid styling | Good. |
| Empty/error states | `empty.rs`, `alert.rs`, `notification.rs`, `skeleton.rs`, `spinner.rs` | Good. |

The calendar does not do what this app needs. The item slot gets a `CalendarItemState`. That state
carries only `kind`, `active`, `in_range`, `muted`, `disabled` and `today`, and no date
(`crates/base/src/calendar.rs`). The label callback gets `(kind, i32)`, so a day cell sees only its
day number. A bank holiday can't be styled per day without abusing the disabled matcher or the
range selection. The calendar is also built for picking dates (selection, range, month/year
pickers), not for showing them. A month grid built by hand from `div`s and `chrono` is cheap,
about 7 columns by ≤6 rows. It keeps full control over holiday and four-day-weekend styling.

### Theming and light/dark

- `gpui_kit::init` calls `Theme::change(ThemeMode::Light, …)` at startup, so the default is light
  (`crates/component/src/theme/mod.rs`). Following the system takes an explicit
  `Theme::sync_system_appearance(Some(window), cx)`. Re-syncing when the appearance changes is the
  app's job (GPUI's window-appearance observer). `ThemeMode` maps from GPUI's `WindowAppearance`,
  vibrant variants included.
- Themes use semantic tokens (`cx.theme().background`, `foreground`, `accent`, `muted_foreground`,
  …), JSON theme files and a registry (`theme/registry.rs`, `theme/schema.rs`,
  `default-theme.json`). Custom colours for "predicted" and "four-day weekend" fit as app-level
  tokens or `Tag::custom`.

### macOS window chrome and app menu

- GPUI: `cx.set_menus(Vec<Menu>)` sets the native menu bar, `set_dock_menu`, `OsAction`
  (cut/copy/paste/undo…) and the Services system menu; `on_reopen`, `hide` and `quit`; and
  `bind_keys` for shortcuts (`crates/gpui/src/app.rs`, `crates/gpui/src/platform/app_menu.rs` in
  zed). `WindowOptions` / `TitlebarOptions` control the titlebar, traffic-light position and
  transparency.
- gpui-kit: `TitleBar` draws a custom title bar with content in it. `TitleBar::window_options()`
  sets up a transparent titlebar with traffic lights at (9, 9) (`title_bar.rs`, example
  `examples/window_title`). `NativeMenu` shows OS-drawn context menus through AppKit `NSMenu`
  (objc2), built from GPUI `MenuItem`s (`native_menu/`). `Root` hosts dialogs, sheets and
  notifications.
- Accessibility: GPUI depends on `accesskit`, and gpui-kit wires AccessKit roles and names into its
  components ([gpui-kit.com](https://gpui-kit.com/)).

### How apps depend on them

There are three routes. All of them pin exactly.

1. **`gpui-kit` from crates.io (recommended by gpui-kit).** `gpui-kit = "0.9.0"`
   ([Getting Started](https://gpui-kit.com/docs/getting-started),
   [Installation](https://gpui-kit.com/docs/installation)). It pulls GPUI through the **`gpui-pre`**
   crates: snapshots of Zed's GPUI crates from a recorded Zed commit (currently `zed@f816e1f`),
   renamed for crates.io. **Longbridge publishes them (`huacnlee`), not Zed.** gpui-kit pins them with
   `=` (`gpui = { package = "gpui-pre", version = "=0.3.11" }`). The comment in the workspace
   [`Cargo.toml`](https://github.com/longbridge/gpui-kit/blob/main/Cargo.toml) explains why: a caret
   requirement once moved apps onto a snapshot gpui-component did not compile against (#3156).
2. **GPUI directly from crates.io.** The `gpui` crate stops at **0.2.2 (2025-10-22)**, more than a
   year stale, and `gpui_platform` was never published (crates.io API). Zed's README example
   (`gpui = "*"`, `gpui_platform = "*"`) therefore does not resolve from crates.io today. The working
   crates.io route to current GPUI is `gpui-pre = "=0.3.11"` plus
   `gpui_platform = { package = "gpui-pre-platform", version = "=0.3.11", features = ["font-kit"] }`.
   **Without `font-kit` macOS renders no text**
   ([Getting Started](https://gpui-kit.com/docs/getting-started)).
3. **Git dependency on `zed-industries/zed`** pinned to a `rev`. This follows Zed main. Every bump
   is a manual API migration with no changelog.

Rust version: Zed's `rust-toolchain.toml` is `1.98.1`, edition 2024. gpui-kit's docs ask for Rust
≥ 1.92. None of the crates declares a `rust-version`. Our `1.99` is ahead of both, which is
fine, but GPUI expects you to track latest stable.

Platform floor: gpui-kit's installation guide says **macOS 15 or later** plus Xcode Command Line
Tools.

Licences: GPUI and gpui-kit are both Apache-2.0. They are compatible with this repo's MIT, but a
distributed bundle should carry their notices.

### Maturity and churn

- gpui-kit ships about **one minor release a week**: 0.6.6 on 2026-09-21 up to 0.9.0 on 2026-11-02,
  with `gpui-pre` bumped in lockstep (crates.io). The v0.8.0 notes alone list about 15 "Breaking
  Changes" sections, among them crate renames and switching to `gpui-pre`
  ([`release-notes.md`](https://github.com/longbridge/gpui-kit/blob/main/release-notes.md)).
  Expect to make code changes on every upgrade. Upgrading rarely and deliberately is cheaper than
  following each release.
- Bus factor: the crates.io route to current GPUI goes through one maintainer's snapshot crates. If
  Longbridge stops publishing, the fallback is a Zed git dependency.
- gpui-kit's docs are much richer than GPUI's: a guide, a component catalogue, a headless UI test
  harness (`gpui_kit::test`, `test-support` feature) and a coding guide.
- Build cost: the default `component` feature pulls a large tree: ropey, lsp-types, markdown, regex,
  notify, rust-i18n, schemars, syntect bits, chrono, smol, objc2. Tree-sitter grammars are optional.
  gpui-kit recommends `[profile.dev.package] gpui-pre = { opt-level = 3 }` because debug builds
  render slowly. Expect first builds to take minutes.
- The workspace dependency gpui-kit publishes enables `runtime_shaders`, `x11` and `wayland` on
  `gpui_platform` (workspace `Cargo.toml`). `runtime_shaders` compiles the Metal shaders at
  launch. Without it, `crates/gpui_apple/build.rs` in zed runs `xcrun metal`/`metallib` at build
  time, which needs full Xcode. That is why gpui-kit only needs the Command Line Tools.
- An optional `gpui-fast` feature switches to Longbridge's fork of the GPUI renderer. It is a
  separate maintenance track. Leave it off.

### Signing and distribution

Neither project bundles, signs or notarises for you, and neither blocks it.

- gpui-kit's [packaging guide](https://github.com/longbridge/gpui-kit/blob/main/website/docs/packaging.md)
  walks through a hand-built `.app` (Info.plist, `.icns`, resources resolved from the bundle),
  Developer ID `codesign --options runtime`, a DMG, `notarytool` and `stapler`. It says to sign
  nested code from the inside out rather than relying on `--deep`. It also points out that
  `cx.set_app_identity(bundle_id, name)` is a runtime identity separate from the bundle id, and both
  should agree.
- Zed does the same thing for its own release with a forked `cargo-bundle`, `codesign` and
  entitlements ([`script/bundle-mac`](https://github.com/zed-industries/zed/blob/main/script/bundle-mac)).
- Default icon assets are embedded in the binary, so a single-binary bundle needs no extra
  resource files.
- Nothing ties the build to a moving dependency at runtime. Exact pins plus a committed
  `Cargo.lock` give reproducible release builds.

## Implications for this app

- **Use `gpui-kit` from crates.io with an exact pin (`gpui-kit = "=0.9.0"`), and use only GPUI
  through `gpui_kit::*`.** It is the only maintained crates.io route to current GPUI. It provides
  segmented tabs, tags, badges, lists, tooltips, theming and a title bar, all of which this app
  wants. Going without it means writing those by hand on `gpui-pre`. That is feasible for an app
  this size, but not cheaper.
- **Build the month grid by hand.** The stock `Calendar` can't style a day by its date. A
  7-column `div` grid fed by the core's holiday data is cheap and expresses "bank holiday",
  "predicted bank holiday" and "four-day weekend" exactly.
- **Keep the core and CLI free of GPUI.** Use a workspace with a `core` crate (pure logic, no
  I/O decisions tied to UI), a `cli` binary and an `app` binary. Only the app pays gpui-kit's
  build cost and churn. gpui-kit already depends on `chrono` 0.4, so a core that uses `chrono`
  adds no second date library to the app build. That is an input to the date-library decision,
  not a decision.
- **Light/dark needs a few explicit lines:** call `sync_system_appearance` after `init` and
  re-sync on window appearance changes. The kit starts in light mode.
- **App menu and shortcuts are first-class** through `cx.set_menus` and `bind_keys`. Keyboard
  navigation goes through GPUI actions and focus handles.
- **Budget for upgrades.** Treat a gpui-kit bump as a deliberate task, not a `cargo update`.
- **Distribution stays open.** It needs macOS 15+, a stable bundle id chosen early (also passed to
  `set_app_identity`), Apache-2.0 notices in the bundle, and the signing steps from gpui-kit's
  packaging guide later.
- **Not verified here:** how GPUI's `Render` signatures and builder chains fare under clippy
  pedantic. The first scaffold will show which lints need a `#[expect(..., reason = ...)]`.
