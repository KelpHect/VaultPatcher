<img src="assets/brand/logo.svg" width="96" alt="Vault Patcher logo: an Eridian Vault arch holding an Eridium crystal">

# Vault Patcher

A desktop patcher, tweaker and mod installer for the Borderlands series, in the spirit of
MarkerPatch (Dead Space 2) and Fallout 76 Quick Configuration. Built in Rust with
[GPUI](https://github.com/zed-industries/zed/tree/main/crates/gpui) and
[gpui-component](https://github.com/longbridge/gpui-component).

**Borderlands 2** and **Borderlands GOTY Enhanced** are fully supported. **The Pre-Sequel** is
in preview (detection, tweaks, presets, SDK install).

## Modes

Toggle between modes from the title bar.

- **Simple** (default): a MarkerPatch-style **One-Click Setup** plus a short **Quick Settings**
  page that saves as you change things. The setup bundle for BL2 includes:
  - Modern Defaults: borderless window at native resolution, framerate matched to your
    monitor, lower input lag, no motion blur or texture pop-in, skipped logos and ads,
    console on `~`.
  - Skip the launcher, and the 4 GB memory patch.
  - DXVK (Vulkan) with a tuned `dxvk.conf`, plus an HD Visual Upgrade.
  - The Python SDK, bug-fix mods (Firing Fix, Automatic Reload Fix), and BL3/BL4-style
    quality of life (vendor refills, auto pickup, loot lights, no ads, quick startup, plus
    opt-in instant vehicles and menu controls). Mods are enabled automatically.

  - **Vault Patcher Community Patch**: a balance-neutral text-mod patch built on the user's
    own PC from pinned upstream files. It takes the Unofficial Community Patch's bug-fix
    section (balance, loot and difficulty changes excluded), apple1417's Text Fixes, and
    Apocalyptech's Sorted Fast Travel, plus Gearbox's official hotfixes. Everything is
    merged into one offline file (`Binaries/VaultPatcher.blcm`) that Text Mod Loader
    auto-runs. Mega TimeSaver XL can be added as an opt-in. Nothing third-party is
    redistributed; see `src/textmod.rs`.

  *Restore vanilla* undoes everything, and every change is backed up first.
- **Advanced**: every tweak, preset, exe patch and mod-manager tool, as described below.

## Comparison images

Settings that Nvidia's tweak guide covers link to its interactive comparison pages. Those
pages are linked, never bundled, because Nvidia's terms don't allow redistribution. Our own
images are captured by the maintainers and published as a release asset
(`comparisons-bl2.zip`), which the app downloads on first run. Anyone can re-shoot them from
**App Settings → Tools → Comparison Capture**, which runs these steps on their own PC:

1. Writes each option of a setting.
2. Launches the game straight into a chosen save (via Quick Startup's `-Character=`).
3. A tiny helper SDK mod closes startup notices, hides the HUD and weapon, and signals ready.
   The app grabs the game window, then the helper quits.
4. The screenshot is stored as a JPEG in `%APPDATA%\VaultPatcher\comparisons`.
5. Settings are restored afterwards.

Settings pages are a searchable list with a detail pane. For settings with images, the
pane shows your current option and any other side by side with a draggable divider; click
a thumbnail to compare against it, or open the full-window viewer (← → switch sides, Esc
closes).

## Features

- **82 config tweaks for BL2** across display, framerate, world detail, AA, textures,
  outlines/cel shading, post-processing, shadows, PhysX, FOV, console, HUD, gameplay, audio,
  startup and network. Each one shows its ini location, default, performance cost, and
  whether the in-game menu also manages it.
- **Search** across every setting (Ctrl+F), with filters for changed and waiting values.
- **Staged edits**: nothing is written until you press *Apply* (Ctrl+S). *Review* lists
  every change as old → new first, and every save offers *Undo*. Applying edits only the
  lines involved, keeps comments, duplicate keys and formatting intact, and keeps the
  launcher's private `LauncherConfig\WillowEngine.ini` in sync.
- **Profiles**: save your current settings under a name, load them later, and export or
  import them as files to share.
- **Health check** on the Overview page (game and settings found, read-only files, 4 GB
  patch, SDK up to date, DXVK support), each with a link to the fix.
- **Presets** (Community Essentials, Clean Look, Pandora Ultra, Balanced, Competitive FPS,
  Potato Mode, Factory Settings).
- **Exe patches**: Large Address Aware plus the four classic BL2/TPS console hex edits.
  Each patch is matched by byte signature, so it's only offered when it matches exactly once.
- **Mod manager**: one-click install and update of the latest Willow2 Python SDK from
  GitHub, SDK mod and text mod install/enable/disable/remove (drop files onto the window,
  or right-click a mod), and core SDK modules locked.
- **Backups**: every apply, patch and SDK install is snapshotted first and can be restored
  in one click.
- **Launch options** on the Overview page (`-NoLauncher`, `-NoStartupMovies` and more),
  used by the sidebar's Play button.
- **Look and feel**: a dense desktop layout (Fluent metrics, Segoe UI, Lucide icons) with
  Borderlands yellow as the accent. The game's own art is read from your PC: the Steam
  library banner and logo head the Setup and Overview pages, and each game's exe icon
  appears in the game switcher. Three palettes (Graphite, Pandora, Hyperion) and optional
  button sounds from the game's launcher audio files (nothing is bundled).
- **Updates and support**: checks for a newer Vault Patcher and mod SDK at startup, and
  *Copy diagnostics* puts a bug-report summary on the clipboard.
- **Detection** of Steam libraries, Epic Games installs, and `Documents\My Games` config
  folders, with manual overrides.

## Build

```sh
cargo run            # debug
cargo build --release
cargo test           # unit tests
cargo test -- --ignored --nocapture   # read-only checks against a local BL2 install
```

`cargo build --release` produces one self-contained `VaultPatcher.exe`: fonts, icons and
the logo are embedded, the C runtime is linked statically (`.cargo/config.toml`), and the app
icon is rendered from `assets/brand/logo.svg` at build time. Everything else (mods, DXVK,
comparison images) is downloaded to the user's PC on demand.

Requires Windows 10/11. Barlow Condensed is bundled under the OFL and the Lucide icons under
ISC (`assets/icons/LICENSE-lucide.txt`); body text uses the system Segoe UI.

## Architecture

```
src/
  core/        game-agnostic: ini engine, Steam/Epic detection, backups, PE/hex patching,
               game art (Steam library cache, exe icons)
  tweaks/      tweak model (Control + Binding), ConfigSet, table builders
  games/       one module per game; willow.rs holds the shared BL2/TPS catalog
  pages/       one module per page kind; each renders from the shared Workspace
  mods.rs      SDK + mod install logic
  patches.rs   exe patch definitions
  workspace.rs shared app state and every mutation
  health.rs    Overview health checks and the diagnostics report
  profiles.rs  named settings profiles (JSON)
  app.rs       window shell: title bar, nav rail, page host, Apply bar, toasts, dialogs
  theme.rs     palettes (mirrored into gpui-component), rarity colors, icons
  ui.rs        widgets: buttons, switches, sliders, chips, setting rows
```

### Adding a tweak
Add one entry to the game's `TWEAKS` table (e.g. `games/willow.rs`) with `toggle`,
`slider`, `choice` or `custom`. Keys listed after the first are mirror copies, written
only if that file exists. Tests check ids, categories and preset references.

### Adding a page
Add a `PageKind` variant, a module in `pages/`, and a match arm in `pages::render`, then
list it in a game's `nav`.

### Adding a game
Create `games/<id>.rs` with a `GameDef` (detection, config folder, ini files, tweaks,
presets, patches, mod support, nav), then register it in `games::all()`.

## Sources

Tweak keys and option values were checked against a live install and the game's
localization files. Background research came from PCGamingWiki, the Nvidia BL2/TPS tweak
guide, OpenBLCMM (`IniTweaksPanel`, `HexDictionary`), the BLCMods Hex-Edits wiki, and the
bl-sdk projects. Not affiliated with Gearbox Software or 2K.

## License

Vault Patcher is free software under the [GNU General Public License v3.0 or later](LICENSE).
Bundled assets keep their own licenses: Barlow Condensed (SIL OFL,
`assets/fonts/Barlow-OFL.txt`) and Lucide icons (ISC, `assets/icons/LICENSE-lucide.txt`).
Borderlands is a trademark of Gearbox Software; this project isn't affiliated with Gearbox
or 2K.
