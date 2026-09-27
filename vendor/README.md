# Vendored crates (our fork)

`gpui` 0.2.2 and `gpui-component` 0.5.1, copied from crates.io and patched in
via `[patch.crates-io]` in the root `Cargo.toml`. Both are Apache-2.0 (see each
crate's `LICENSE-APACHE`). Upstream examples, tests and docs were dropped to
keep the repo small. macOS is not supported: the Metal backend
(`platform/mac`) was removed, and the app refuses to build there.

Changes are kept small and marked with doc comments, so they can be reapplied
on a newer upstream or offered back. When upgrading, diff against the pristine
crate from `~/.cargo/registry/src/*/gpui-<version>`.

## gpui

- **Alpha blending over Mica** (`directx_renderer.rs`): the main blend state
  composites alpha "over" (`a + d(1 - a)`) instead of adding it. The
  swapchain is premultiplied over the Mica backdrop, and additive alpha made
  stacked translucent fills (a card on the content layer) turn opaque grey.

- **Backdrop blur** (Fluent acrylic): `Styled::backdrop_blur(px)` /
  `Style::backdrop_blur`, painted before the element's shadow and background
  through `Window::paint_backdrop_blur`. It adds a `Backdrop` scene primitive
  that is ordered like any other, so it blurs exactly what was painted under it.
  - DirectX (`platform/windows`): the frame so far is copied, blurred
    horizontally into a scratch texture (`backdrop_blur_x`), then vertically
    back into the frame inside the rounded rect (`backdrop_blur_y`) with 2%
    noise. Dual-source blending replaces the pixels by coverage; the frame is
    mostly transparent over Mica, so blending over it would keep the sharp
    original showing. Only the region the two passes read is copied (each
    backdrop plus three blur radii), and the scratch textures are created on
    first use, resized with the window, freed after 3 s without a backdrop
    and dropped on device loss.
  - Blade (Linux, `platform/blade`): swapchain images can't be sampled, so a
    frame with backdrops is drawn into an offscreen texture and copied to the
    drawable at the end (frames without them are unchanged). Each backdrop
    batch blurs horizontally into a scratch texture, then vertically back
    inside the rounded rect with the same noise. Blade has no dual-source
    blending, so the replace is two draws per backdrop: scale the frame by
    `1 - coverage`, then add the blurred color times coverage. Targets are
    created on first use and dropped on resize.
- **Scale transforms**: `Styled::transform_scale(f32)` /
  `Window::with_element_scale(origin, scale, f)`. Every primitive goes through
  `Window::insert_primitive`, which applies the current `ElementTransform`
  (quads, borders, shadows, backdrops, paths, underlines, text, icons, images).
  Text and SVG icons are re-rasterized at the scaled size (in 1% steps, so an
  animation reuses a few atlas entries) and stay sharp; images stretch. Layout
  and hit testing are unchanged, so it's meant for animations (dialog entrance
  1.05 → 1).
- **Hover and press fades**: elements with hover/active styles fade their
  solid background to the new color over 83 ms (WinUI's brush transition),
  from wherever the fade is, so quick moves don't jump. Targets are only
  taken once hover is known (prepaint/paint), so a hovered element settles
  instead of flipping between its layout and paint colors every frame.
- **Frames on demand** (Windows): the vsync thread only wakes windows while
  `FRAME_WANTED` is set: by a dirtied view, `on_next_frame` (animations),
  input, or the one-second present after input. An idle window no longer runs
  a frame callback at the monitor's refresh rate.
- **Frame-rate cap** (Windows): `gpui::set_max_frame_rate(Some(fps))` lets the
  vsync thread start a frame only every `1/fps` (with half a vblank of slack,
  so a divisor of the refresh rate lands on exact vblanks). Animations are
  time-based and still finish on time; for 100 ms after input frames aren't
  capped, so dragging and typing stay at the display's rate.
- **Bounded image caches**: `gpui::retain_recent(id, n)` keeps only the `n`
  most recently drawn images (never one drawn in the last 250 ms, so a frame
  can't evict its own) and frees the rest with their atlas textures.
- **Reduce motion**: `gpui::set_reduce_motion` / `gpui::reduce_motion()`
  turns the fades off and lets components skip their animations.
- **Focus visuals**: `InteractiveElement::focus_visible(style)` applies only
  when focus arrived by keyboard (`Window::is_focus_visible`; set by Tab
  navigation, cleared by any mouse press), and `Styled::outline` draws a ring
  outside an element without affecting layout, with an optional inner ring.
- **Inert elements**: `InteractiveElement::inert(bool)` for disabled
  controls: hover/pressed styles, pointer/key/drag handlers and the Tab stop
  are dropped; tooltips still show so they can say why.
- **Text truncation** re-measures at the final width: the layout cache now
  keys on the truncation width too, so `truncate()` ends with an ellipsis
  instead of clipping mid-glyph after a min-content pass.
- **Image caches free their textures**: a `RetainAllImageCache` released
  mid-draw (the usual case) defers dropping its images until every window is
  back in `App::windows`, so the atlas textures are actually removed.
- Fixed two float-literal inference warnings in `taffy.rs`.
- `stacksafe` 1.0 (0.1 pulled in `proc-macro-error2`, which a future Rust
  will reject).

## gpui-component

- `StyledExt::popover_style` (menus, dropdowns, popovers) and tooltips are
  acrylic: a 30px backdrop blur under the theme's (translucent) `popover`
  color, with `radius_lg` overlay corners on flyouts.
- Medium inputs are 32px tall (WinUI TextBox), whatever the rem size.
- Popup menus open with a flyout entrance (fade over 83 ms while settling
  from 96% size), skipped under `gpui::reduce_motion()`.
- Text inputs notify only when their bounds, cursor, selection or scroll
  changed. Upstream notified from every paint, which kept any window with a
  text box repainting at the monitor's refresh rate.
- `Button::content_fill()` lets the content row fill the button, left
  aligned, for ComboBox-style faces.
