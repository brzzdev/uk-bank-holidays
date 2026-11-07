# Following the system light/dark appearance in GPUI and gpui-kit

- Status: findings
- Date: 2026-11-07
- Question: [#15](https://github.com/brzzdev/uk-bank-holidays/issues/15)

## Context

The app builds on `gpui-kit = "=0.9.0"` from crates.io and reaches GPUI only through `gpui_kit::*`
(decided in [#3](https://github.com/brzzdev/uk-bank-holidays/issues/3)). gpui-kit 0.9.0 depends on
`gpui-component` 0.9.0, `gpui-base` 0.9.0 and the `gpui-pre` 0.3.11 family, a snapshot of Zed's GPUI
at `zed@f816e1f`. On macOS, `gpui-pre-platform` pulls in `gpui-pre-macos` 0.3.11. Every finding below
comes from those published crate sources, plus the `v0.9.0` tag of
[longbridge/gpui-kit](https://github.com/longbridge/gpui-kit/tree/v0.9.0).

The month grid is drawn by hand. It marks bank holidays, predicted bank holidays (a dashed red
circle), four-day weekends and bridges. gpui-kit starts in its light theme.

The question: how to read the macOS appearance at launch, how to observe it changing while the app
runs, how to switch gpui-kit's theme, and whether the hand-drawn grid follows on its own.

## Findings

### Reading the appearance at launch

GPUI offers two readings, both returning `WindowAppearance` (`Light`, `VibrantLight`, `Dark`,
`VibrantDark`; `gpui-pre/src/platform.rs`):

- `App::window_appearance()` asks the platform for the application's appearance
  (`gpui-pre/src/app.rs`). On macOS this is `NSApplication.effectiveAppearance`
  (`gpui-pre-macos/src/platform.rs`). It needs no window, so it works before the first window opens.
- `Window::appearance()` returns the window's cached appearance (`gpui-pre/src/window.rs`). The cache
  is filled from `NSWindow.effectiveAppearance` when the window is created and refreshed on every
  change (`gpui-pre-macos/src/window.rs`).

`window_appearance_from_native` maps the AppKit names `aqua`, `darkAqua`, `vibrantLight` and
`vibrantDark`. Any other name, or none at all, falls back to `Light`
(`gpui-pre-macos/src/window_appearance.rs`).

### Switching gpui-kit's theme

- `gpui_kit::init` → `gpui_component::theme::init` calls `Theme::change(ThemeMode::Light, None, cx)`,
  so every app starts light whatever the system says (`gpui-component/src/theme/mod.rs`).
- `Theme::sync_system_appearance(window: Option<&mut Window>, cx)` reads `window.appearance()` when it
  gets a window and `cx.window_appearance()` otherwise, then calls `Theme::change`. A comment there
  prefers the window reading because the app-level one errors on Linux. That doesn't affect macOS.
- `Theme::change(mode: impl Into<ThemeMode>, …)` loads the registered light or dark `ThemeConfig`,
  pushes it down to the `gpui-base` layer (`sync_base`) and calls `cx.refresh_windows()`. The window
  argument is ignored. `ThemeMode` has only `Light` and `Dark`. `From<WindowAppearance>` folds the
  vibrant variants into those two (`gpui-component/src/theme/mod.rs`).
- `Theme::global(cx).is_dark()` / `cx.theme().mode` tell the current mode.

### Observing changes while running

gpui-kit 0.9.0 itself never subscribes to appearance changes. No call to `observe_window_appearance`
appears anywhere in `gpui-kit`, `gpui-component` or `gpui-base`. The app has to wire it up.

- AppKit calls `viewDidChangeEffectiveAppearance` on GPUI's view. That runs the window's
  `on_appearance_changed` callback (`gpui-pre-macos/src/window.rs`).
- GPUI defers the callback onto the foreground executor, because AppKit can call it synchronously
  while `App` is already borrowed. It then updates the cached `Window::appearance` and runs the
  window's appearance observers (`gpui-pre/src/window.rs`). The test
  `test_appearance_change_runs_after_app_update` pins the deferral down: an observer sees the change
  only after the current update finishes.
- Two ways to subscribe, both returning a `Subscription` that ends when dropped unless `.detach()`ed:
  - `Window::observe_window_appearance(|window, cx| …)` (`gpui-pre/src/window.rs`)
  - `Context<T>::observe_window_appearance(window, |view, window, cx| …)`, which dies with the view
    entity (`gpui-pre/src/app/context.rs`)
- The observers are per window. There's no app-level "system appearance changed" event, so an
  observer needs an open window. A one-window app has nothing to miss.
- gpui-kit's own showcase uses this exact pattern. Its root view calls
  `cx.observe_window_appearance(window, |this, window, cx| { …; cx.notify(); }).detach()` in its
  constructor (`crates/base/examples/showcase/mod.rs` at `v0.9.0`).

Put together, following the system takes about this much code in the root view's constructor (shape
only, not compiled here):

```rust
Theme::sync_system_appearance(Some(window), cx);
cx.observe_window_appearance(window, |_, window, cx| {
    Theme::sync_system_appearance(Some(window), cx);
})
.detach();
```

Syncing inside the window's build closure, before the first frame, avoids a light flash on a dark
system. Calling `Theme::sync_system_appearance(None, cx)` right after `gpui_kit::init`, from
`cx.window_appearance()`, does the same.

### Forcing an appearance (an override)

`App::set_window_appearance(Option<WindowAppearance>)` sets `NSApplication.appearance`. `Some(_)`
forces light or dark for the native chrome (titlebar, window edges) of every window. `None` clears
the override and follows the system again (`gpui-pre/src/app.rs`, `gpui-pre-macos/src/platform.rs`).
Its doc comment says windows stop tracking system changes while an appearance is forced. It is a
no-op on other platforms.

An inference from AppKit's appearance inheritance, not verified by running: once
`NSApplication.appearance` is set, each window's `effectiveAppearance` follows it, and the
appearance observer above fires. So `sync_system_appearance` would follow a forced appearance too,
and the chrome and the gpui-kit theme stay in step without a second code path. If the app only
called `Theme::change(ThemeMode::Dark, …)`, the components would turn dark while the native titlebar
followed the system.

### Do hand-drawn elements follow the theme?

Only if they read their colours from the theme while rendering.

- `Theme::change` ends in `cx.refresh_windows()`. That marks every window dirty with
  `refreshing = true` (`gpui-pre/src/app.rs`), and while refreshing, views skip their render cache
  (`gpui-pre/src/view.rs`). So every view's `render` runs again after a theme switch, including
  `.cached()` ones, with no `cx.notify()` needed.
- An element styled with `cx.theme().<token>` in `render` picks up the new value. An `Hsla` built
  from literals (`hsla(…)`, `rgb(0x…)`) or kept in view state stays the same in both modes.
- The theme gives the grid plenty of semantic tokens: `background`, `foreground`, `muted`,
  `muted_foreground`, `border`, `accent`, `primary`, `danger`, `success`, `warning`, `info`,
  `chart_1`…`chart_5`. Base colours are `red`, `red_light`, `green`, `blue`, `yellow`, `magenta` and
  `cyan`, each with a `_light` variant. All are `Hsla` on `ThemeColor`, reached through
  `Deref` from `Theme` (`gpui-component/src/theme/theme_color.rs`, `gpui-component/src/theme/mod.rs`).
  These tokens change per mode. In `default-theme.json`, `base.red` is `red-600` in "Default Light"
  and `red-400` in "Default Dark", and `danger.background` is `red-500` and `red-400`
  (`gpui-component/src/theme/default-theme.json`).
- GPUI draws dashed borders natively: `Styled::border_dashed()` sets `BorderStyle::Dashed`
  (`gpui-pre/src/styled.rs`, `gpui-pre/src/scene.rs`). A rounded `div` with
  `.border_dashed().border_color(cx.theme().red)` makes a predicted-holiday circle whose red adapts
  per mode.
- If the app wants its own colours, such as a specific bridge tint, it can branch on
  `cx.theme().is_dark()` in `render`, or take them from a custom `ThemeConfig`
  (`gpui-component/src/theme/schema.rs`, `registry.rs`). Either way, the render path has to re-read
  them.
- GPUI also ships its own small `Colors::for_appearance(window)` palette (`gpui-pre/src/colors.rs`).
  It is independent of gpui-kit's theme. Using it alongside `cx.theme()` would mean two sources of
  truth.

### Testing

GPUI's test platform has `TestWindow::simulate_appearance_change(WindowAppearance)`
(`gpui-pre/src/platform/test/window.rs`), as used in the deferral test above. Whether it is reachable
through gpui-kit's `test-support` harness (`gpui_kit::test`) was not checked.

## Implications for this app

- **Following the system takes about four lines:** sync once before the first frame, then re-sync
  from `observe_window_appearance` on the main window. gpui-kit does neither for us.
- **The grid follows automatically if it reads theme tokens in `render`.** Literal colours won't
  change. The predicted-holiday dashed circle can use `cx.theme().red` (or `danger`) with
  `border_dashed()`, and its red already differs between light and dark.
- **Colours with no matching token need a choice:** branch on `is_dark()`, or add them to a custom
  theme. Both work. That is a design decision, not a platform limit.
- **An in-app override, if the product wants one, has a native hook:** `set_window_appearance`
  keeps the titlebar and the theme in step. Whether to offer one is for the follow-system vs
  override grilling ticket, not this note.
- **Not verified by running:** the override inference above, and how the vibrant appearances look
  when folded into the light/dark themes.
