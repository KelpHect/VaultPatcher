# Vendored crates (our fork)

`gpui` 0.2.2 and `gpui-component` 0.5.1, copied from crates.io and patched in
via `[patch.crates-io]` in the root `Cargo.toml`. Both are Apache-2.0 (see each
crate's `LICENSE-APACHE`). Upstream examples, tests and docs were dropped to
keep the repo small.

Changes are kept small and marked with doc comments, so they can be reapplied
on a newer upstream or offered back. When upgrading, diff against the pristine
crate from `~/.cargo/registry/src/*/gpui-<version>`.

## gpui

- **Backdrop blur** (Fluent acrylic): `Styled::backdrop_blur(px)` /
  `Style::backdrop_blur`, painted before the element's shadow and background
  through `Window::paint_backdrop_blur`. It adds a `Backdrop` scene primitive
  that is ordered like any other, so it blurs exactly what was painted under it.
  - DirectX (`platform/windows`): the frame so far is copied, blurred
    horizontally into a scratch texture (`backdrop_blur_x`), then vertically
    back into the frame inside the rounded rect (`backdrop_blur_y`) with 2%
    noise. Dual-source blending replaces the pixels by coverage; the frame is
    mostly transparent over Mica, so blending over it would keep the sharp
    original showing. Scratch textures are created on first use, resized with
    the window and dropped on device loss.
  - Blade (Linux) and Metal (macOS) skip backdrops for now; elements keep their
    translucent fill.
- **Scale transforms**: `Styled::transform_scale(f32)` /
  `Window::with_element_scale(origin, scale, f)`. Every primitive goes through
  `Window::insert_primitive`, which applies the current `ElementTransform`
  (quads, borders, shadows, backdrops, paths, underlines, text, icons, images).
  Sprites stretch their rasterized tile, and layout and hit testing are
  unchanged, so it's meant for short animations (dialog entrance 1.05 → 1).
- Fixed two float-literal inference warnings in `taffy.rs`.

## gpui-component

- `StyledExt::popover_style` (menus, dropdowns, popovers) and tooltips are
  acrylic: a 30px backdrop blur under the theme's (translucent) `popover`
  color, with `radius_lg` overlay corners on flyouts.
