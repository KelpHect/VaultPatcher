//! Shared application state. Pages hold an `Entity<Workspace>`, read from it
//! while rendering, and call its methods to change anything; every mutation
//! notifies observers so all pages stay in sync.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::PathBuf;
use std::time::Duration;

use anyhow::{Context as _, Result, anyhow};
use gpui::Context;
use serde::{Deserialize, Serialize};

use crate::core::backup::{self, Backup};
use crate::core::binpatch::PatchState;
use crate::core::detect::{self, Install, Store};
use crate::games::{self, GameDef, Mode, PageKind};
use crate::mods::{self, ModEntry, SdkStatus};
use crate::patches;
use crate::setup::{self, ComponentKind, Status};
use crate::tweaks::{ConfigSet, Preset, Tweak, Value};

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct AppSettings {
    pub active_game: Option<String>,
    pub manual_installs: HashMap<String, PathBuf>,
    pub manual_config_dirs: HashMap<String, PathBuf>,
    /// Mark ini files read-only after applying so the game can't revert them.
    pub lock_configs_after_apply: bool,
    /// Launch switches the user ticked, per game.
    pub launch_args: HashMap<String, Vec<String>>,
    pub max_backups: Option<usize>,
    pub mode: Mode,
    pub palette: crate::theme::Palette,
    pub sound_muted: bool,
    /// Advanced: show ini file/section/key under each setting.
    pub show_file_details: bool,
    /// Loop the launcher's menu music.
    pub music: bool,
    /// The first-run welcome has been dismissed.
    pub welcomed: bool,
}

impl AppSettings {
    fn path() -> PathBuf {
        backup::data_dir().join("settings.json")
    }

    pub fn load() -> Self {
        std::fs::read_to_string(Self::path())
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    pub fn save(&self) {
        let path = Self::path();
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let _ = std::fs::write(path, serde_json::to_vec_pretty(self).unwrap_or_default());
    }
}

/// The comparison viewer: two options of one setting, split by a slider.
#[derive(Clone, Copy, Debug)]
pub struct Preview {
    pub tweak: &'static str,
    /// Image indices shown left and right of the divider.
    pub left: usize,
    pub right: usize,
    /// Divider position, 0..=1 from the left edge.
    pub split: f32,
}

/// Which tweaks the tweak pages list.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TweakFilter {
    All,
    /// Differs from the game's shipped default (on disk or staged).
    Modified,
    Staged,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StepState {
    Queued,
    Running,
    Done(String),
    Failed(String),
}

/// Progress of a Simple-mode setup (or restore) run.
pub struct SetupRun {
    pub uninstall: bool,
    pub steps: Vec<(&'static str, StepState)>,
    pub finished: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToastKind {
    Info,
    Success,
    Error,
}

/// A button on a toast.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToastAction {
    /// Put back the values the last save replaced.
    Undo,
}

#[derive(Clone, Debug)]
pub struct Toast {
    pub id: u64,
    pub kind: ToastKind,
    pub message: String,
    pub action: Option<ToastAction>,
}

/// What the last settings write replaced, for "Undo".
pub struct Undo {
    game: usize,
    values: Vec<(&'static str, Value)>,
}

/// Newer versions found online (checked once at startup).
#[derive(Default)]
pub struct Updates {
    /// (tag, release page) of a newer Vault Patcher.
    pub app: Option<(String, String)>,
    /// Latest mod SDK release tag.
    pub sdk: Option<String>,
}

#[derive(Clone, Debug, Default)]
pub struct ExeInfo {
    pub size: u64,
    pub patch_states: HashMap<&'static str, PatchState>,
}

pub struct GameState {
    pub def: &'static GameDef,
    pub install: Option<Install>,
    /// Logo, banner and icon found on this PC (Steam cache / exe).
    pub art: crate::core::art::Art,
    pub config_dir: Option<PathBuf>,
    pub config: ConfigSet,
    pub pending: BTreeMap<&'static str, Value>,
    pub exe: Option<ExeInfo>,
    pub sdk: SdkStatus,
    pub mods: Vec<ModEntry>,
    pub backups: Vec<Backup>,
    /// Components ticked on the setup page.
    pub setup_selected: BTreeSet<&'static str>,
    /// Setup statuses touch the disk; cached until the next refresh/write.
    status_cache: std::cell::RefCell<HashMap<&'static str, Status>>,
    /// Last "is the game running" answer and when it was taken.
    running_cache: std::cell::Cell<Option<(std::time::Instant, bool)>>,
    /// Saved profiles, re-read when they change.
    pub profiles: Vec<crate::profiles::Entry>,
}

impl GameState {
    fn new(def: &'static GameDef) -> Self {
        Self {
            def,
            install: None,
            art: Default::default(),
            config_dir: None,
            config: ConfigSet::default(),
            pending: BTreeMap::new(),
            exe: None,
            sdk: SdkStatus::NotInstalled,
            mods: Vec::new(),
            backups: Vec::new(),
            setup_selected: def.setup.iter().filter(|c| c.recommended).map(|c| c.id).collect(),
            status_cache: Default::default(),
            running_cache: Default::default(),
            profiles: crate::profiles::list(def.id),
        }
    }

    fn invalidate_statuses(&self) {
        self.status_cache.borrow_mut().clear();
    }

    /// True while the game's exe is running (it rewrites its ini files on
    /// exit, and its exe/DLLs are locked).
    pub fn is_running(&self) -> bool {
        // Rendering asks often; the process list only needs checking every
        // couple of seconds.
        if let Some((at, running)) = self.running_cache.get()
            && at.elapsed() < Duration::from_secs(2)
        {
            return running;
        }
        let running = self.exe_path().is_some_and(|p| crate::compare::game_running(&p));
        self.running_cache.set(Some((std::time::Instant::now(), running)));
        running
    }

    fn reload_profiles(&mut self) {
        self.profiles = crate::profiles::list(self.def.id);
    }

    pub fn config_found(&self) -> bool {
        self.config.files().any(|(_, f)| f.exists)
    }

    /// The value currently written in the game's files.
    pub fn current(&self, tweak: &Tweak) -> Option<Value> {
        tweak.read(&self.config)
    }

    /// The value the UI should show: pending edit, else file, else default.
    pub fn effective(&self, tweak: &Tweak) -> Value {
        self.pending
            .get(tweak.id)
            .cloned()
            .or_else(|| self.current(tweak))
            .unwrap_or_else(|| tweak.default.to_value())
    }

    pub fn exe_path(&self) -> Option<PathBuf> {
        self.install.as_ref().map(|i| i.root.join(self.def.exe))
    }
}

pub struct Workspace {
    pub games: Vec<GameState>,
    pub active: usize,
    pub page: PageKind,
    pub settings: AppSettings,
    pub toasts: Vec<Toast>,
    /// Label of a long-running background job, if any.
    pub busy: Option<String>,
    pub tweak_filter: TweakFilter,
    pub setup_run: Option<SetupRun>,
    /// Setup rows showing their full description.
    pub setup_expanded: BTreeSet<&'static str>,
    /// Comparison lightbox: (tweak id, image index).
    pub preview: Option<Preview>,
    /// A running (or finished) comparison capture.
    pub capture: Option<std::sync::Arc<std::sync::Mutex<crate::compare::CaptureProgress>>>,
    pub capture_save: Option<String>,
    pub capture_settle: u32,
    /// Settings search text (from the toolbar search box).
    pub search: String,
    /// The setting shown in the detail pane.
    pub selected_tweak: Option<&'static str>,
    /// The detail pane's comparison: (tweak, image on the right, divider 0..=1).
    pub inline_compare: Option<(&'static str, usize, f32)>,
    /// Advanced: the "review changes" dialog is open.
    pub review_open: bool,
    pub updates: Updates,
    /// Shared text inputs, created by the window (they need one).
    pub search_input: Option<gpui::Entity<gpui_component::input::InputState>>,
    pub profile_input: Option<gpui::Entity<gpui_component::input::InputState>>,
    undo: Option<Undo>,
    next_toast: u64,
    /// Per game: bumped on every Simple-mode edit; a pending auto-apply only
    /// runs if no newer edit for that game arrived in the meantime.
    autoapply_generation: Vec<u64>,
}

impl Workspace {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let settings = AppSettings::load();
        let games: Vec<GameState> = games::all().iter().map(|d| GameState::new(d)).collect();
        let active = settings
            .active_game
            .as_ref()
            .and_then(|id| games.iter().position(|g| g.def.id == id))
            .unwrap_or(0);
        let page = match settings.mode {
            Mode::Simple => PageKind::Setup,
            Mode::Advanced => PageKind::Overview,
        };
        let mut ws = Self {
            games,
            active,
            page,
            settings,
            toasts: Vec::new(),
            busy: None,
            tweak_filter: TweakFilter::All,
            setup_run: None,
            setup_expanded: BTreeSet::new(),
            preview: None,
            capture: None,
            capture_save: None,
            capture_settle: 25,
            search: String::new(),
            selected_tweak: None,
            inline_compare: None,
            review_open: false,
            updates: Updates::default(),
            search_input: None,
            profile_input: None,
            undo: None,
            next_toast: 0,
            autoapply_generation: vec![0; games::all().len()],
        };
        for i in 0..ws.games.len() {
            ws.refresh_game(i);
        }
        ws.fetch_comparison_packs(cx);
        ws.check_updates(cx);
        let probe = cx.background_executor().spawn(async { crate::core::gpu::dxvk_ready().is_ok() });
        cx.spawn(async move |this, cx| {
            let ready = probe.await;
            this.update(cx, |ws, cx| {
                for game in &mut ws.games {
                    if !ready {
                        game.setup_selected.remove("dxvk");
                    }
                    game.invalidate_statuses();
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
        // UI sounds come from the first installed Willow game's launcher.
        let audio_dir = ws
            .games
            .iter()
            .filter_map(|g| g.install.as_ref())
            .map(|i| i.root.join(crate::sound::AUDIO_DIR))
            .find(|d| d.join("ButtonClick.mp3").is_file());
        crate::sound::init(audio_dir.as_deref(), ws.settings.sound_muted, ws.settings.music);
        cx.notify();
        ws
    }

    /// Downloads published comparison images for games that don't have
    /// them yet. Silent if nothing is published or there's no network.
    fn fetch_comparison_packs(&mut self, cx: &mut Context<Self>) {
        let missing: Vec<&'static str> = self
            .games
            .iter()
            .filter(|g| !g.def.comparisons.is_empty() && !crate::compare::has_local_images(g.def.id))
            .map(|g| g.def.id)
            .collect();
        let all: Vec<&'static str> = self.games.iter().filter(|g| !g.def.comparisons.is_empty()).map(|g| g.def.id).collect();
        let task = cx.background_executor().spawn(async move {
            let downloaded = missing.iter().filter(|id| crate::compare::download_pack(id).is_ok()).count();
            // Small copies for the settings pages (only missing ones are made).
            let previews: usize = all.iter().map(|id| crate::compare::make_previews(id)).sum();
            downloaded + previews
        });
        cx.spawn(async move |this, cx| {
            if task.await > 0 {
                this.update(cx, |_, cx| cx.notify()).ok();
            }
        })
        .detach();
    }

    pub fn game(&self) -> &GameState {
        &self.games[self.active]
    }

    pub fn game_mut(&mut self) -> &mut GameState {
        &mut self.games[self.active]
    }

    pub fn select_game(&mut self, index: usize, cx: &mut Context<Self>) {
        if index < self.games.len() {
            if self.active != index {
                crate::sound::play(crate::sound::Sound::Whoosh);
            }
            self.active = index;
            if !self.games[index].def.nav_items(self.settings.mode).any(|n| n.kind == self.page) {
                self.page = self.home_page();
            }
            self.settings.active_game = Some(self.games[index].def.id.to_string());
            self.settings.save();
            cx.notify();
        }
    }

    pub fn navigate(&mut self, page: PageKind, cx: &mut Context<Self>) {
        // Pages that only exist in the other mode (e.g. One-Click Setup from
        // Advanced) switch modes, unless that would strand waiting changes.
        let def = self.game().def;
        let other = match self.settings.mode {
            Mode::Simple => Mode::Advanced,
            Mode::Advanced => Mode::Simple,
        };
        if page != PageKind::Capture && !def.nav_items(self.settings.mode).any(|n| n.kind == page) && def.nav_items(other).any(|n| n.kind == page) {
            if !self.game().pending.is_empty() {
                self.toast(ToastKind::Info, "Apply or discard the waiting changes first", cx);
                return;
            }
            self.settings.mode = other;
            self.settings.save();
        }
        if self.page != page {
            crate::sound::play(crate::sound::Sound::Whoosh);
        }
        self.page = page;
        cx.notify();
    }

    pub fn set_muted(&mut self, muted: bool, cx: &mut Context<Self>) {
        crate::sound::set_muted(muted);
        crate::sound::set_music(self.settings.music && !muted);
        self.settings.sound_muted = muted;
        self.settings.save();
        cx.notify();
    }

    pub fn set_music(&mut self, on: bool, cx: &mut Context<Self>) {
        crate::sound::set_music(on && !self.settings.sound_muted);
        self.settings.music = on;
        self.settings.save();
        cx.notify();
    }

    pub fn mode(&self) -> Mode {
        self.settings.mode
    }

    fn home_page(&self) -> PageKind {
        match self.settings.mode {
            Mode::Simple => PageKind::Setup,
            Mode::Advanced => PageKind::Overview,
        }
    }

    pub fn set_palette(&mut self, palette: crate::theme::Palette, cx: &mut Context<Self>) {
        crate::theme::set_palette(palette);
        crate::theme::apply(cx);
        self.settings.palette = palette;
        self.settings.save();
        cx.notify();
    }

    pub fn set_mode(&mut self, mode: Mode, cx: &mut Context<Self>) {
        if self.settings.mode == mode {
            return;
        }
        self.settings.mode = mode;
        self.settings.save();
        crate::sound::play(crate::sound::Sound::Whoosh);
        if !self.game().def.nav_items(mode).any(|n| n.kind == self.page) {
            self.page = self.home_page();
        }
        cx.notify();
    }

    // ---- detection & loading -------------------------------------------------

    pub fn refresh_game(&mut self, index: usize) {
        let settings = self.settings.clone();
        let game = &mut self.games[index];
        let def = game.def;

        game.install = settings
            .manual_installs
            .get(def.id)
            .filter(|p| p.join(def.exe).is_file())
            .map(|p| Install {
                root: p.clone(),
                store: Store::Manual,
            })
            .or_else(|| detect::detect(&def.detect_spec()));
        game.art = crate::core::art::find(def.id, def.steam_app_ids, game.exe_path().as_deref());

        game.config_dir = settings
            .manual_config_dirs
            .get(def.id)
            .cloned()
            .or_else(|| def.default_config_dir());
        game.config = match &game.config_dir {
            Some(dir) => ConfigSet::load(dir, def.ini_files),
            None => ConfigSet::default(),
        };
        // Drop pending edits that now match the file.
        let config = &game.config;
        game.pending.retain(|id, v| {
            def.tweak(id)
                .is_some_and(|t| t.read(config).as_ref() != Some(v))
        });

        game.exe = game.exe_path().filter(|p| p.is_file()).map(|path| scan_exe(def, &path));
        game.invalidate_statuses();

        match (def.mods, &game.install) {
            (Some(support), Some(install)) => {
                game.sdk = mods::sdk_status(def.id, support, &install.root);
                game.mods = mods::scan(support, &install.root);
            }
            _ => {
                game.sdk = SdkStatus::NotInstalled;
                game.mods.clear();
            }
        }
        game.backups = backup::list(def.id);
    }

    pub fn refresh_active(&mut self, cx: &mut Context<Self>) {
        self.refresh_game(self.active);
        cx.notify();
    }

    pub fn set_manual_install(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        let def = self.game().def;
        match detect::validate_manual(&path, def.exe) {
            Some(root) => {
                self.settings.manual_installs.insert(def.id.to_string(), root);
                self.settings.save();
                self.refresh_active(cx);
                self.toast(ToastKind::Success, format!("{} install set", def.short), cx);
            }
            None => self.toast(
                ToastKind::Error,
                format!("Couldn't find {} under that folder", def.exe),
                cx,
            ),
        }
    }

    pub fn set_manual_config_dir(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        let def = self.game().def;
        self.settings.manual_config_dirs.insert(def.id.to_string(), path);
        self.settings.save();
        self.refresh_active(cx);
        self.toast(ToastKind::Success, "Config folder set", cx);
    }

    pub fn clear_manual_paths(&mut self, cx: &mut Context<Self>) {
        let id = self.game().def.id;
        self.settings.manual_installs.remove(id);
        self.settings.manual_config_dirs.remove(id);
        self.settings.save();
        self.refresh_active(cx);
        self.toast(ToastKind::Info, "Back to auto-detected paths", cx);
    }

    // ---- tweaks ----------------------------------------------------------------

    pub fn pending_count(&self) -> usize {
        self.game().pending.len()
    }

    pub fn stage(&mut self, tweak: &'static Tweak, value: Value, cx: &mut Context<Self>) {
        let value = tweak.clamp(value);
        let game = self.game_mut();
        let on_disk = game.current(tweak).unwrap_or_else(|| tweak.default.to_value());
        if value == on_disk {
            game.pending.remove(tweak.id);
        } else {
            game.pending.insert(tweak.id, value);
        }
        cx.notify();
    }

    /// Loads a preset into the pending changes, replacing what was there.
    pub fn stage_preset(&mut self, preset: &Preset, cx: &mut Context<Self>) {
        self.game_mut().pending.clear();
        let def = self.game().def;
        if preset.values.is_empty() {
            // An empty preset means "factory settings" for everything except
            // the monitor-specific display choices.
            for tweak in def.visible_tweaks() {
                if !matches!(tweak.id, "resolution" | "window_mode" | "fullscreen") {
                    self.stage(tweak, tweak.default.to_value(), cx);
                }
            }
        }
        for (id, value) in preset.values {
            if let Some(tweak) = def.tweak(id) {
                self.stage(tweak, value.to_value(), cx);
            }
        }
        let count = self.game().pending.len();
        self.toast(
            ToastKind::Info,
            format!("{} loaded — {count} change(s) waiting. Press Apply to write them.", preset.name),
            cx,
        );
    }

    pub fn stage_defaults(&mut self, categories: &[&str], cx: &mut Context<Self>) {
        let def = self.game().def;
        for tweak in def.visible_tweaks().filter(|t| categories.contains(&t.category)) {
            self.stage(tweak, tweak.default.to_value(), cx);
        }
    }

    pub fn discard_pending(&mut self, cx: &mut Context<Self>) {
        self.game_mut().pending.clear();
        cx.notify();
    }

    pub fn apply_pending(&mut self, cx: &mut Context<Self>) {
        match self.try_apply_pending(self.active, false) {
            Ok(n) => {
                self.review_open = false;
                self.toast_with(ToastKind::Success, format!("Applied {n} change(s)"), Some(ToastAction::Undo), cx);
            }
            Err(e) => self.toast(ToastKind::Error, format!("Apply failed: {e:#}"), cx),
        }
    }

    fn try_apply_pending(&mut self, gi: usize, coalesce: bool) -> Result<usize> {
        let pending = std::mem::take(&mut self.games[gi].pending);
        let def = self.games[gi].def;
        // What's on disk now, so the save can be undone.
        let previous: Vec<(&'static str, Value)> = pending
            .keys()
            .filter_map(|id| def.tweak(id))
            .map(|t| (t.id, self.games[gi].current(t).unwrap_or_else(|| t.default.to_value())))
            .collect();
        let label = if coalesce {
            QUICK_LABEL.to_string()
        } else {
            format!("Before applying {} change(s)", pending.len())
        };
        let result = self.write_configs(gi, &label, coalesce, |config| {
            for (id, value) in &pending {
                if let Some(tweak) = def.tweak(id) {
                    tweak.write(config, value);
                }
            }
        });
        match result {
            Ok(()) => {
                // A burst of Quick Settings saves undoes back to before the burst.
                match &mut self.undo {
                    Some(undo) if coalesce && undo.game == gi => {
                        for (id, v) in previous {
                            if !undo.values.iter().any(|(i, _)| *i == id) {
                                undo.values.push((id, v));
                            }
                        }
                    }
                    _ => self.undo = Some(Undo { game: gi, values: previous }),
                }
                Ok(pending.len())
            }
            Err(e) => {
                self.games[gi].pending = pending;
                Err(e)
            }
        }
    }

    /// Writes back the values the last save replaced.
    pub fn undo_last(&mut self, cx: &mut Context<Self>) {
        let Some(undo) = self.undo.take() else {
            return;
        };
        let def = self.games[undo.game].def;
        let result = self.write_configs(undo.game, "Before undo", false, |config| {
            for (id, value) in &undo.values {
                if let Some(tweak) = def.tweak(id) {
                    tweak.write(config, value);
                }
            }
        });
        match result {
            Ok(()) => self.toast(ToastKind::Info, format!("Undone: {} setting(s) put back", undo.values.len()), cx),
            Err(e) => self.toast(ToastKind::Error, format!("Couldn't undo: {e:#}"), cx),
        }
    }

    /// Drops one waiting change (from the review dialog).
    pub fn unstage(&mut self, id: &str, cx: &mut Context<Self>) {
        self.game_mut().pending.remove(id);
        if self.game().pending.is_empty() {
            self.review_open = false;
        }
        cx.notify();
    }

    pub fn set_review(&mut self, open: bool, cx: &mut Context<Self>) {
        self.review_open = open && !self.game().pending.is_empty();
        cx.notify();
    }

    pub fn set_search(&mut self, text: String, cx: &mut Context<Self>) {
        if self.search != text {
            self.search = text;
            cx.notify();
        }
    }

    /// Updates the detail pane's comparison for `tweak`.
    pub fn set_inline(&mut self, tweak: &'static str, right: Option<usize>, split: Option<f32>, cx: &mut Context<Self>) {
        let (_, r, s) = self.inline_compare.filter(|c| c.0 == tweak).unwrap_or((tweak, usize::MAX, 0.5));
        self.inline_compare = Some((tweak, right.unwrap_or(r), split.unwrap_or(s).clamp(0., 1.)));
        cx.notify();
    }

    pub fn select_tweak(&mut self, id: &'static str, cx: &mut Context<Self>) {
        if self.selected_tweak != Some(id) {
            self.selected_tweak = Some(id);
            cx.notify();
        }
    }

    // ---- profiles ------------------------------------------------------------

    /// Saves every setting's current value (including waiting changes) as a
    /// named profile.
    pub fn save_profile(&mut self, name: &str, cx: &mut Context<Self>) -> bool {
        let name = name.trim();
        if name.is_empty() {
            self.toast(ToastKind::Error, "Give the profile a name first", cx);
            return false;
        }
        let game = self.game();
        let profile = crate::profiles::snapshot(game.def, name, |t| game.effective(t));
        let path = crate::profiles::path_for(game.def.id, name);
        match crate::profiles::write(&profile, &path) {
            Ok(()) => {
                self.game_mut().reload_profiles();
                self.toast(ToastKind::Success, format!("Saved profile \u{201c}{name}\u{201d}"), cx);
                true
            }
            Err(e) => {
                self.toast(ToastKind::Error, format!("Couldn't save the profile: {e:#}"), cx);
                false
            }
        }
    }

    /// Loads a profile: staged in Advanced mode, saved at once in Simple.
    pub fn load_profile(&mut self, path: &std::path::Path, cx: &mut Context<Self>) {
        let def = self.game().def;
        let loaded = crate::profiles::read(path).and_then(|p| crate::profiles::resolve(def, &p).map(|r| (p.name, r)));
        let (name, (values, skipped)) = match loaded {
            Ok(v) => v,
            Err(e) => {
                self.toast(ToastKind::Error, format!("{e:#}"), cx);
                return;
            }
        };
        self.game_mut().pending.clear();
        for (tweak, value) in values {
            self.stage(tweak, value, cx);
        }
        let changes = self.game().pending.len();
        let note = if skipped > 0 { format!(" ({skipped} unknown setting(s) skipped)") } else { String::new() };
        if self.mode() == Mode::Simple {
            self.schedule_autoapply(cx);
            self.toast(ToastKind::Info, format!("Loading {name}: {changes} change(s){note}"), cx);
        } else {
            self.toast(ToastKind::Info, format!("{name} loaded: {changes} change(s) waiting{note}"), cx);
        }
    }

    /// Copies a profile file into this game's profile folder.
    pub fn import_profile(&mut self, path: &std::path::Path, cx: &mut Context<Self>) {
        let def = self.game().def;
        let result = crate::profiles::read(path).and_then(|p| {
            crate::profiles::resolve(def, &p)?;
            crate::profiles::write(&p, &crate::profiles::path_for(def.id, &p.name))?;
            Ok(p.name)
        });
        self.game_mut().reload_profiles();
        match result {
            Ok(name) => self.toast(ToastKind::Success, format!("Imported {name}"), cx),
            Err(e) => self.toast(ToastKind::Error, format!("Import failed: {e:#}"), cx),
        }
    }

    pub fn export_profile(&mut self, from: &std::path::Path, to: &std::path::Path, cx: &mut Context<Self>) {
        match std::fs::copy(from, to) {
            Ok(_) => self.toast(ToastKind::Success, format!("Exported to {}", to.display()), cx),
            Err(e) => self.toast(ToastKind::Error, format!("Export failed: {e}"), cx),
        }
    }

    pub fn delete_profile(&mut self, path: &std::path::Path, cx: &mut Context<Self>) {
        if let Err(e) = std::fs::remove_file(path) {
            self.toast(ToastKind::Error, format!("{e}"), cx);
        }
        self.game_mut().reload_profiles();
        cx.notify();
    }

    // ---- updates -------------------------------------------------------------

    /// One background check per launch: a newer Vault Patcher, and the latest
    /// mod SDK. Silent without a network.
    fn check_updates(&mut self, cx: &mut Context<Self>) {
        let sdk_repo = self.games.iter().find_map(|g| g.def.mods).map(|m| m.sdk_repo);
        let task = cx.background_executor().spawn(async move {
            let app = crate::core::net::latest_release(crate::compare::IMAGE_REPO)
                .ok()
                .filter(|(tag, _)| crate::core::net::is_newer(tag, env!("CARGO_PKG_VERSION")));
            let sdk = sdk_repo.and_then(|r| crate::core::net::latest_release(r).ok()).map(|(tag, _)| tag);
            Updates { app, sdk }
        });
        cx.spawn(async move |this, cx| {
            let updates = task.await;
            this.update(cx, |ws, cx| {
                ws.updates = updates;
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    /// The newer SDK release, when the installed one is outdated.
    pub fn sdk_update_available(&self) -> Option<&str> {
        match (&self.game().sdk, self.updates.sdk.as_deref()) {
            (SdkStatus::Installed(v), Some(latest)) if v != latest => Some(latest),
            _ => None,
        }
    }

    /// Edits game `gi`'s configs with `edit` and writes them, after:
    /// refusing while the game runs (it rewrites them on exit), re-reading
    /// the files from disk (so changes made in the game's own menus aren't
    /// reverted), keeping a permanent "original settings" snapshot, and
    /// snapshotting the files about to change. With `coalesce`, one snapshot
    /// covers a burst of Quick Settings edits instead of one per edit.
    fn write_configs(&mut self, gi: usize, label: &str, coalesce: bool, edit: impl FnOnce(&mut ConfigSet)) -> Result<()> {
        let lock = self.settings.lock_configs_after_apply;
        let max_backups = self.settings.max_backups;
        let game = &mut self.games[gi];
        let def = game.def;
        let Some(dir) = game.config_dir.clone().filter(|_| game.config_found()) else {
            return Err(anyhow!(
                "config files not found — launch the game once or set the folder in App Settings"
            ));
        };
        if game.is_running() {
            return Err(anyhow!(
                "{} is running — close it first, or it will overwrite these settings when it exits",
                def.name
            ));
        }
        let mut config = ConfigSet::load(&dir, def.ini_files);
        let existing: Vec<PathBuf> = config.files().filter(|(_, f)| f.exists).map(|(_, f)| f.path.clone()).collect();
        if !game.backups.iter().any(|b| b.label == backup::ORIGINAL_LABEL) {
            backup::create(def.id, backup::ORIGINAL_LABEL, &existing).context("saving your original settings")?;
        }
        edit(&mut config);
        let dirty = config.dirty_paths();
        if dirty.is_empty() {
            game.config = config;
            return Ok(());
        }
        let recent = game.backups.first().is_some_and(|b| {
            b.label == label
                && b.dir.metadata().and_then(|m| m.modified()).is_ok_and(|t| {
                    t.elapsed().unwrap_or_default() < Duration::from_secs(5 * 60)
                })
        });
        if !(coalesce && recent) {
            backup::create(def.id, label, &dirty).context("creating backup")?;
        }
        let written = config.save_dirty();
        game.config = ConfigSet::load(&dir, def.ini_files);
        game.invalidate_statuses();
        game.backups = backup::list(def.id);
        let written = written?;
        if lock {
            for path in &written {
                backup::set_readonly(path, true)?;
            }
        }
        prune_backups(def.id, max_backups);
        game.backups = backup::list(def.id);
        Ok(())
    }

    /// Simple mode: stage a change and write it shortly after the user stops
    /// adjusting, so a slider drag doesn't produce a snapshot per pixel.
    pub fn set_now(&mut self, tweak: &'static Tweak, value: Value, cx: &mut Context<Self>) {
        self.stage(tweak, value, cx);
        self.schedule_autoapply(cx);
    }

    pub fn preset_now(&mut self, preset: &Preset, cx: &mut Context<Self>) {
        let def = self.game().def;
        for (id, value) in preset.values {
            if let Some(tweak) = def.tweak(id) {
                self.stage(tweak, value.to_value(), cx);
            }
        }
        self.schedule_autoapply(cx);
    }

    fn schedule_autoapply(&mut self, cx: &mut Context<Self>) {
        let gi = self.active;
        self.autoapply_generation[gi] += 1;
        let generation = self.autoapply_generation[gi];
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(Duration::from_millis(700)).await;
            this.update(cx, |ws, cx| {
                // Only this game's edits, and only if no newer edit came in.
                if ws.autoapply_generation[gi] != generation || ws.games[gi].pending.is_empty() {
                    return;
                }
                match ws.try_apply_pending(gi, true) {
                    Ok(_) => {
                        ws.toast_with(ToastKind::Success, "Saved", Some(ToastAction::Undo), cx);
                    }
                    Err(e) => ws.toast(ToastKind::Error, format!("Couldn't save: {e:#}"), cx),
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    // ---- one-click setup ---------------------------------------------------------

    pub fn component_status(&self, component: &setup::Component) -> Status {
        self.component_status_for(self.active, component)
    }

    pub(crate) fn component_status_for(&self, gi: usize, component: &setup::Component) -> Status {
        let game = &self.games[gi];
        if let Some(s) = game.status_cache.borrow().get(component.id) {
            return s.clone();
        }
        let status = setup::status(component, game, &self.launch_args_for(gi));
        game.status_cache.borrow_mut().insert(component.id, status.clone());
        status
    }

    pub fn toggle_component(&mut self, id: &'static str, cx: &mut Context<Self>) {
        let selected = &mut self.game_mut().setup_selected;
        if !selected.remove(id) {
            selected.insert(id);
        }
        cx.notify();
    }

    pub fn toggle_expanded(&mut self, id: &'static str, cx: &mut Context<Self>) {
        if !self.setup_expanded.remove(id) {
            self.setup_expanded.insert(id);
        }
        cx.notify();
    }

    /// Opens the viewer with the clicked option on the right and, on the
    /// left, the game's default (or the first other option).
    pub fn open_preview(&mut self, tweak: &'static str, index: usize, cx: &mut Context<Self>) {
        let def = self.game().def;
        let left = def
            .tweak(tweak)
            .and_then(|t| {
                let default = t.default.to_value();
                crate::compare::images(def.id, t)
                    .iter()
                    .zip(crate::compare::captured_values(def.id, t))
                    .position(|(_, v)| v == default)
            })
            .filter(|&d| d != index)
            .unwrap_or(if index == 0 { 1 } else { 0 });
        self.preview = Some(Preview { tweak, left, right: index, split: 0.5 });
        cx.notify();
    }

    pub fn set_preview(&mut self, f: impl FnOnce(&mut Preview), cx: &mut Context<Self>) {
        if let Some(p) = &mut self.preview {
            f(p);
            cx.notify();
        }
    }

    pub fn close_preview(&mut self, cx: &mut Context<Self>) {
        self.preview = None;
        cx.notify();
    }

    /// Save files the capture tool can load into, newest first.
    pub fn capture_saves(&self) -> Vec<String> {
        let Some(dir) = self.game().config_dir.as_ref().and_then(|d| d.parent()).map(|d| d.join("SaveData")) else {
            return Vec::new();
        };
        let mut saves: Vec<(std::time::SystemTime, String)> = std::fs::read_dir(dir)
            .into_iter()
            .flatten()
            .flatten()
            .filter(|e| e.path().is_dir())
            .flat_map(|profile| std::fs::read_dir(profile.path()).into_iter().flatten().flatten())
            .filter_map(|e| {
                let name = e.file_name().to_string_lossy().into_owned();
                let lower = name.to_ascii_lowercase();
                if !(lower.starts_with("save") && lower.ends_with(".sav")) {
                    return None;
                }
                Some((e.metadata().ok()?.modified().ok()?, name))
            })
            .collect();
        saves.sort_by_key(|s| std::cmp::Reverse(s.0));
        saves.into_iter().map(|(_, n)| n).collect()
    }

    pub fn capture_running(&self) -> bool {
        self.capture
            .as_ref()
            .is_some_and(|p| p.lock().is_ok_and(|p| !p.finished))
    }

    pub fn start_capture(&mut self, cx: &mut Context<Self>) {
        use crate::compare::{self, CaptureProgress, CaptureRequest};
        if self.capture_running() {
            return;
        }
        let game = self.game();
        let def = game.def;
        let (Some(install), Some(config_dir)) = (game.install.clone(), game.config_dir.clone()) else {
            self.toast(ToastKind::Error, "Game install or config folder not found", cx);
            return;
        };
        let quick_startup = install.root.join("sdk_mods").join("quick_startup.sdkmod");
        if !matches!(game.sdk, SdkStatus::Installed(_) | SdkStatus::Detected) || !quick_startup.is_file() {
            self.toast(ToastKind::Error, "Capture needs the Python SDK and the Quick Startup mod — install them from One-Click Setup first", cx);
            return;
        }
        let Some(save) = self.capture_save.clone().or_else(|| self.capture_saves().into_iter().next()) else {
            self.toast(ToastKind::Error, "No save files found to load into", cx);
            return;
        };
        let shots: Vec<_> = def
            .comparisons
            .iter()
            .filter(|c| c.capture)
            .filter_map(|c| def.tweak(c.tweak))
            .flat_map(|t| compare::capture_values(t).into_iter().map(move |(v, l)| (t, v, l)))
            .collect();
        let progress = std::sync::Arc::new(std::sync::Mutex::new(CaptureProgress {
            total: shots.len(),
            ..Default::default()
        }));
        let request = CaptureRequest {
            game_id: def.id,
            exe: install.root.join(def.exe),
            root: install.root,
            config_dir,
            ini_files: def.ini_files,
            save,
            settle_seconds: self.capture_settle,
            shots,
        };
        self.capture = Some(progress.clone());
        cx.notify();
        let task = cx.background_executor().spawn({
            let progress = progress.clone();
            async move { compare::run(request, progress) }
        });
        // Repaint periodically so progress shows while the game runs.
        let tick_progress = progress.clone();
        cx.spawn(async move |this, cx| loop {
            cx.background_executor().timer(Duration::from_millis(700)).await;
            let finished = tick_progress.lock().map(|p| p.finished).unwrap_or(true);
            if this.update(cx, |_, cx| cx.notify()).is_err() || finished {
                break;
            }
        })
        .detach();
        cx.spawn(async move |this, cx| {
            let result = task.await;
            this.update(cx, |ws, cx| {
                let id = ws.game().def.id;
                ws.game_mut().backups = backup::list(id);
                match result {
                    Ok(n) => ws.toast(ToastKind::Success, format!("Captured {n} image(s). Your settings were restored."), cx),
                    Err(e) => ws.toast(ToastKind::Error, format!("Capture stopped: {e:#}"), cx),
                }
            })
            .ok();
        })
        .detach();
    }

    pub fn cancel_capture(&mut self, cx: &mut Context<Self>) {
        if let Some(p) = &self.capture
            && let Ok(mut p) = p.lock() {
                p.cancel = true;
            }
        cx.notify();
    }

    pub fn setup_running(&self) -> bool {
        self.setup_run.as_ref().is_some_and(|r| !r.finished)
    }

    /// Installs the selected components that aren't active yet (in list
    /// order), or with `uninstall` removes every active component.
    pub fn run_setup(&mut self, uninstall: bool, cx: &mut Context<Self>) {
        if self.setup_running() {
            return;
        }
        let game = self.game();
        let def = game.def;
        let wanted = |c: &setup::Component| {
            let status = self.component_status(c);
            if uninstall {
                // Only undo what Vault Patcher itself installed or applied.
                setup::installed_by_us(c, game, &self.launch_args())
            } else {
                game.setup_selected.contains(c.id) && !status.is_active() && !matches!(status, Status::Blocked(_))
            }
        };
        let mut ids: Vec<&'static str> = def.setup.iter().filter(|c| wanted(c)).map(|c| c.id).collect();
        if !uninstall {
            // Pull in dependencies that aren't installed yet (e.g. the SDK
            // for SDK mods), keeping list order.
            let needed: Vec<&'static str> = ids
                .iter()
                .filter_map(|id| def.component(id))
                .flat_map(|c| c.requires.iter().copied())
                .filter(|dep| !ids.contains(dep))
                .filter(|dep| def.component(dep).is_some_and(|d| !self.component_status(d).is_active()))
                .collect();
            if !needed.is_empty() {
                ids = def
                    .setup
                    .iter()
                    .map(|c| c.id)
                    .filter(|id| ids.contains(id) || needed.contains(id))
                    .collect();
            }
        }
        if uninstall {
            // Remove dependents (SDK mods) before what they depend on.
            ids.reverse();
        }
        if ids.is_empty() {
            self.toast(
                ToastKind::Info,
                if uninstall {
                    "Nothing to restore — no Vault Patcher components are active."
                } else {
                    "Everything selected is already installed."
                },
                cx,
            );
            return;
        }
        self.setup_run = Some(SetupRun {
            uninstall,
            steps: ids.iter().map(|id| (*id, StepState::Queued)).collect(),
            finished: false,
        });
        cx.notify();

        let game_index = self.active;
        cx.spawn(async move |this, cx| {
            for (step, id) in ids.iter().enumerate() {
                // Local work runs right here; downloads come back as a job for
                // the background thread so the UI stays responsive.
                let job = match this.update(cx, |ws, cx| {
                    let job = ws.start_component(game_index, id, uninstall);
                    if job.is_some() {
                        ws.set_step(step, StepState::Running, cx);
                    }
                    job
                }) {
                    Ok(job) => job,
                    Err(_) => return,
                };
                let Some(job) = job else { continue };
                let outcome = match job {
                    Ok(Job::Done(msg)) => Ok(msg),
                    Ok(Job::Background(work)) => cx.background_executor().spawn(async move { work() }).await,
                    Err(e) => Err(e),
                };
                let failed = outcome.is_err();
                this.update(cx, |ws, cx| {
                    ws.set_step(
                        step,
                        match outcome {
                            Ok(msg) => StepState::Done(msg),
                            Err(e) => StepState::Failed(format!("{e:#}")),
                        },
                        cx,
                    );
                    ws.refresh_game(game_index);
                    // Don't install mods on top of a failed SDK, etc.
                    if failed && !uninstall {
                        ws.skip_dependents(game_index, id, cx);
                    }
                })
                .ok();
            }
            this.update(cx, |ws, cx| {
                let failed = ws.setup_run.as_ref().map_or(0, |r| {
                    r.steps.iter().filter(|(_, s)| matches!(s, StepState::Failed(_))).count()
                });
                if let Some(run) = &mut ws.setup_run {
                    run.finished = true;
                }
                ws.refresh_game(game_index);
                match (failed, uninstall) {
                    (0, false) => ws.toast(ToastKind::Success, "All set — enjoy the upgraded game!", cx),
                    (0, true) => ws.toast(ToastKind::Success, "Restored to vanilla", cx),
                    (n, _) => ws.toast(
                        ToastKind::Error,
                        format!("{n} step(s) didn't finish — see the list for details"),
                        cx,
                    ),
                }
            })
            .ok();
        })
        .detach();
    }

    fn set_step(&mut self, step: usize, state: StepState, cx: &mut Context<Self>) {
        if let Some(s) = self.setup_run.as_mut().and_then(|r| r.steps.get_mut(step)) {
            s.1 = state;
        }
        cx.notify();
    }

    fn skip_dependents(&mut self, gi: usize, failed: &str, cx: &mut Context<Self>) {
        let def = self.games[gi].def;
        if let Some(run) = &mut self.setup_run {
            for (id, state) in &mut run.steps {
                let depends = def.component(id).is_some_and(|c| c.requires.contains(&failed));
                if depends && *state == StepState::Queued {
                    *state = StepState::Failed(format!("Skipped: needs {failed}"));
                }
            }
        }
        cx.notify();
    }

    /// Begins one setup step. `None` means the step was already skipped.
    fn start_component(&mut self, game_index: usize, id: &str, uninstall: bool) -> Option<Result<Job>> {
        let skipped = self.setup_run.as_ref().is_some_and(|run| {
            run.steps.iter().any(|(s, st)| *s == id && matches!(st, StepState::Failed(_)))
        });
        if skipped {
            return None;
        }
        let def = self.games[game_index].def;
        let component = def.component(id)?;
        Some(self.component_job(game_index, component, uninstall))
    }

    fn component_job(&mut self, gi: usize, component: &'static setup::Component, uninstall: bool) -> Result<Job> {
        let def = self.games[gi].def;
        let root = self.games[gi].install.as_ref().map(|i| i.root.clone());
        let touches_game = !matches!(component.kind, ComponentKind::LaunchArg(_));
        if touches_game && self.games[gi].is_running() {
            return Err(anyhow!("{} is running — close it first", def.name));
        }
        let need_root = || root.clone().context("game install not found");
        let (game_id, comp_id) = (def.id, component.id);
        Ok(match (component.kind, uninstall) {
            (ComponentKind::Settings { values, display }, false) => {
                let mode = crate::core::display::primary();
                self.write_configs(gi, &format!("Before {}", component.name), false, |config| {
                    for (id, v) in values {
                        if let Some(t) = def.tweak(id) {
                            t.write(config, &v.to_value());
                        }
                    }
                    if let (Some(hook), Some(mode)) = (display, mode) {
                        hook(config, mode);
                    }
                })?;
                setup::record_settings_applied(game_id, comp_id)?;
                Job::Done(match (display, mode) {
                    (Some(_), Some(m)) => format!("Tuned for {}×{} @ {} Hz", m.width, m.height, m.refresh_hz),
                    _ => "Applied".into(),
                })
            }
            (ComponentKind::Settings { values, .. }, true) => {
                self.write_configs(gi, &format!("Before removing {}", component.name), false, |config| {
                    for (id, _) in values {
                        if let Some(t) = def.tweak(id) {
                            t.write(config, &t.default.to_value());
                        }
                    }
                })?;
                crate::core::manifest::remove(game_id, comp_id);
                Job::Done("Back to game defaults".into())
            }
            (ComponentKind::ExePatch(patch), uninstall) => {
                let p = def.patches.iter().find(|p| p.id == patch).context("unknown patch")?;
                if uninstall && !p.revertible {
                    return Ok(Job::Done("Kept (the game ships with it)".into()));
                }
                let path = self.games[gi].exe_path().context("game install not found")?;
                let mut bytes = patches::read_exe(&path)?;
                p.set(&mut bytes, !uninstall)?;
                backup::create(game_id, &format!("Before {}", p.name), std::slice::from_ref(&path))?;
                backup::clear_readonly(&path);
                std::fs::write(&path, bytes)
                    .with_context(|| format!("writing {} (is the game running?)", path.display()))?;
                Job::Done(if uninstall { "Reverted" } else { "Patched" }.into())
            }
            (ComponentKind::LaunchArg(arg), remove) => {
                let mut args = self.launch_args_for(gi);
                args.retain(|a| !a.eq_ignore_ascii_case(arg));
                if !remove {
                    args.push(arg.to_string());
                }
                self.settings.launch_args.insert(game_id.to_string(), args);
                self.settings.save();
                Job::Done(if remove { "Removed" } else { "Added to Play" }.into())
            }
            (ComponentKind::Dxvk { target, exe_dir, conf }, false) => {
                let root = need_root()?;
                Job::Background(Box::new(move || {
                    setup::install_dxvk(game_id, comp_id, &root, target, exe_dir, conf).map(|v| format!("DXVK {v}"))
                }))
            }
            (ComponentKind::Sdk, false) => {
                let root = need_root()?;
                let support = def.mods.context("no SDK for this game")?;
                Job::Background(Box::new(move || {
                    mods::install_sdk_latest(game_id, support, &root).map(|v| format!("SDK {v}"))
                }))
            }
            (ComponentKind::Sdk, true) => {
                let support = def.mods.context("no SDK for this game")?;
                mods::uninstall_sdk(game_id, support, &need_root()?)?;
                Job::Done("Removed".into())
            }
            (ComponentKind::File { url, dest, enable }, false) => {
                let root = need_root()?;
                Job::Background(Box::new(move || setup::install_file(game_id, comp_id, &root, url, dest, enable)))
            }
            (ComponentKind::SdkZip { url, folder }, false) => {
                let root = need_root()?;
                Job::Background(Box::new(move || setup::install_sdk_zip(game_id, comp_id, &root, url, folder)))
            }
            (ComponentKind::TextPatch { game, gearbox_url, .. }, uninstall) => {
                // Every text patch component shares one merged file, so
                // adding or removing one rebuilds it from the rest.
                let root = need_root()?;
                let mut ids = setup::text_patch_parts(game_id);
                ids.retain(|id| id != comp_id);
                if !uninstall {
                    ids.push(comp_id.to_string());
                }
                let parts: Vec<(&'static str, &'static [setup::TextSource])> = def
                    .setup
                    .iter()
                    .filter(|c| ids.iter().any(|id| id == c.id))
                    .filter_map(|c| match c.kind {
                        ComponentKind::TextPatch { sources, .. } => Some((c.id, sources)),
                        _ => None,
                    })
                    .collect();
                Job::Background(Box::new(move || setup::rebuild_text_patch(game_id, &root, game, gearbox_url, &parts)))
            }
            (ComponentKind::Dxvk { .. } | ComponentKind::File { .. } | ComponentKind::SdkZip { .. }, true) => {
                setup::uninstall_files(game_id, comp_id, &need_root()?)?;
                Job::Done("Removed".into())
            }
        })
    }

    pub fn set_config_lock(&mut self, locked: bool, cx: &mut Context<Self>) {
        let paths: Vec<PathBuf> = self
            .game()
            .config
            .files()
            .filter(|(_, f)| f.exists)
            .map(|(_, f)| f.path.clone())
            .collect();
        let result: Result<()> = paths.iter().try_for_each(|p| backup::set_readonly(p, locked));
        match result {
            Ok(()) => self.toast(
                ToastKind::Success,
                if locked {
                    "Config files locked (read-only)"
                } else {
                    "Config files unlocked"
                },
                cx,
            ),
            Err(e) => self.toast(ToastKind::Error, format!("{e:#}"), cx),
        }
        cx.notify();
    }

    pub fn configs_locked(&self) -> bool {
        let files: Vec<_> = self.game().config.files().filter(|(_, f)| f.exists).collect();
        !files.is_empty() && files.iter().all(|(_, f)| backup::is_readonly(&f.path))
    }

    // ---- exe patches -------------------------------------------------------------

    pub fn set_exe_patch(&mut self, patch_id: &str, enabled: bool, cx: &mut Context<Self>) {
        let result = (|| -> Result<()> {
            let game = self.game();
            if game.is_running() {
                return Err(anyhow!("{} is running — close it first", game.def.name));
            }
            let path = game.exe_path().context("game install not found")?;
            let patch = game
                .def
                .patches
                .iter()
                .find(|p| p.id == patch_id)
                .context("unknown patch")?;
            let mut bytes = patches::read_exe(&path)?;
            patch.set(&mut bytes, enabled)?;
            let verb = if enabled { "Before applying" } else { "Before reverting" };
            backup::create(game.def.id, &format!("{verb} {}", patch.name), std::slice::from_ref(&path))?;
            backup::clear_readonly(&path);
            std::fs::write(&path, bytes).with_context(|| {
                format!("writing {} (is the game running?)", path.display())
            })?;
            Ok(())
        })();
        match result {
            Ok(()) => {
                self.refresh_active(cx);
                self.toast(
                    ToastKind::Success,
                    if enabled { "Patch applied" } else { "Patch reverted" },
                    cx,
                );
            }
            Err(e) => self.toast(ToastKind::Error, format!("{e:#}"), cx),
        }
    }

    // ---- backups -----------------------------------------------------------------

    pub fn backup_configs_now(&mut self, cx: &mut Context<Self>) {
        let game = self.game();
        let paths: Vec<PathBuf> = game
            .config
            .files()
            .filter(|(_, f)| f.exists)
            .map(|(_, f)| f.path.clone())
            .collect();
        match backup::create(game.def.id, "Manual config snapshot", &paths) {
            Ok(_) => {
                self.refresh_active(cx);
                self.toast(ToastKind::Success, "Snapshot saved", cx);
            }
            Err(e) => self.toast(ToastKind::Error, format!("{e:#}"), cx),
        }
    }

    /// Restores a backup (identified by its folder, not a list position),
    /// snapshotting the current files first so the restore can be undone.
    pub fn restore_backup(&mut self, dir: PathBuf, cx: &mut Context<Self>) {
        let Some(b) = backup::load(&dir) else {
            self.toast(ToastKind::Error, "That backup no longer exists", cx);
            return;
        };
        if self.game().is_running() {
            let name = self.game().def.name;
            self.toast(ToastKind::Error, format!("{name} is running — close it first"), cx);
            return;
        }
        let current: Vec<PathBuf> = b.files.iter().map(|f| f.original.clone()).collect();
        let result = backup::create(self.game().def.id, &format!("Before restoring \"{}\"", b.label), &current)
            .context("backing up the current files")
            .and_then(|_| backup::restore(&b));
        match result {
            Ok(()) => {
                self.refresh_active(cx);
                self.toast(ToastKind::Success, format!("Restored \"{}\"", b.label), cx);
            }
            Err(e) => self.toast(ToastKind::Error, format!("Restore failed: {e:#}"), cx),
        }
    }

    pub fn delete_backup(&mut self, dir: PathBuf, cx: &mut Context<Self>) {
        let Some(b) = backup::load(&dir) else {
            return;
        };
        match backup::delete(&b) {
            Ok(()) => self.refresh_active(cx),
            Err(e) => self.toast(ToastKind::Error, format!("{e:#}"), cx),
        }
    }

    // ---- mods --------------------------------------------------------------------

    pub fn install_sdk(&mut self, cx: &mut Context<Self>) {
        let game = self.game();
        let (Some(support), Some(install)) = (game.def.mods, game.install.clone()) else {
            return;
        };
        let game_id = game.def.id;
        self.busy = Some(format!("Downloading {}…", support.sdk_name));
        cx.notify();
        let task = cx
            .background_executor()
            .spawn(async move { mods::install_sdk_latest(game_id, support, &install.root) });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            this.update(cx, |ws, cx| {
                ws.busy = None;
                ws.refresh_active(cx);
                match result {
                    Ok(version) => ws.toast(
                        ToastKind::Success,
                        format!("{} {version} installed", support.sdk_name),
                        cx,
                    ),
                    Err(e) => ws.toast(ToastKind::Error, format!("SDK install failed: {e:#}"), cx),
                }
            })
            .ok();
        })
        .detach();
    }

    pub fn install_sdk_from_zip(&mut self, zip: PathBuf, cx: &mut Context<Self>) {
        let game = self.game();
        let (Some(support), Some(install)) = (game.def.mods, game.install.clone()) else {
            return;
        };
        let result = mods::install_sdk_zip(game.def.id, support, &install.root, &zip);
        self.refresh_active(cx);
        match result {
            Ok(()) => self.toast(ToastKind::Success, format!("{} installed", support.sdk_name), cx),
            Err(e) => self.toast(ToastKind::Error, format!("{e:#}"), cx),
        }
    }

    pub fn uninstall_sdk(&mut self, cx: &mut Context<Self>) {
        let game = self.game();
        let (Some(support), Some(install)) = (game.def.mods, game.install.clone()) else {
            return;
        };
        let result = mods::uninstall_sdk(game.def.id, support, &install.root);
        self.refresh_active(cx);
        match result {
            Ok(()) => self.toast(ToastKind::Success, "Mod manager removed (your mods were kept)", cx),
            Err(e) => self.toast(ToastKind::Error, format!("{e:#}"), cx),
        }
    }

    pub fn install_mod_files(&mut self, files: Vec<PathBuf>, cx: &mut Context<Self>) {
        let game = self.game();
        let (Some(support), Some(install)) = (game.def.mods, game.install.clone()) else {
            return;
        };
        let mut ok = 0;
        for file in files {
            match mods::install_mod(support, &install.root, &file) {
                Ok(()) => ok += 1,
                Err(e) => self.toast(ToastKind::Error, format!("{e:#}"), cx),
            }
        }
        self.refresh_active(cx);
        if ok > 0 {
            self.toast(ToastKind::Success, format!("Installed {ok} mod(s)"), cx);
        }
    }

    pub fn set_mod_enabled(&mut self, path: PathBuf, enabled: bool, cx: &mut Context<Self>) {
        let game = self.game();
        let (Some(support), Some(install)) = (game.def.mods, game.install.clone()) else {
            return;
        };
        let Some(entry) = game.mods.iter().find(|m| m.path == path).cloned() else {
            return;
        };
        if let Err(e) = mods::set_enabled(support, &install.root, &entry, enabled) {
            self.toast(ToastKind::Error, format!("{e:#}"), cx);
        }
        self.refresh_active(cx);
    }

    pub fn remove_mod(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        let Some(entry) = self.game().mods.iter().find(|m| m.path == path).cloned() else {
            return;
        };
        match mods::remove(&entry) {
            Ok(()) => self.toast(ToastKind::Success, format!("Removed {}", entry.name), cx),
            Err(e) => self.toast(ToastKind::Error, format!("{e:#}"), cx),
        }
        self.refresh_active(cx);
    }

    // ---- launch ------------------------------------------------------------------

    pub fn launch_args(&self) -> Vec<String> {
        self.launch_args_for(self.active)
    }

    pub(crate) fn launch_args_for(&self, gi: usize) -> Vec<String> {
        let def = self.games[gi].def;
        self.settings
            .launch_args
            .get(def.id)
            .cloned()
            .unwrap_or_else(|| {
                def.launch_args
                    .iter()
                    .filter(|a| a.default_on)
                    .map(|a| a.arg.to_string())
                    .collect()
            })
    }

    pub fn toggle_launch_arg(&mut self, arg: &str, cx: &mut Context<Self>) {
        let mut args = self.launch_args();
        if let Some(i) = args.iter().position(|a| a == arg) {
            args.remove(i);
        } else {
            args.push(arg.to_string());
        }
        let id = self.game().def.id.to_string();
        self.settings.launch_args.insert(id, args);
        self.settings.save();
        self.game().invalidate_statuses();
        cx.notify();
    }

    pub fn launch(&mut self, cx: &mut Context<Self>) {
        let Some(exe) = self.game().exe_path() else {
            self.toast(ToastKind::Error, "Game install not found", cx);
            return;
        };
        let args = self.launch_args();
        let result = std::process::Command::new(&exe)
            .args(&args)
            .current_dir(exe.parent().unwrap_or(&exe))
            .spawn();
        match result {
            Ok(_) => self.toast(ToastKind::Success, format!("Launching {}…", self.game().def.short), cx),
            Err(e) => self.toast(ToastKind::Error, format!("Launch failed: {e}"), cx),
        }
    }

    // ---- misc --------------------------------------------------------------------

    pub fn update_settings(&mut self, f: impl FnOnce(&mut AppSettings), cx: &mut Context<Self>) {
        f(&mut self.settings);
        self.settings.save();
        cx.notify();
    }

    pub fn toast(&mut self, kind: ToastKind, message: impl Into<String>, cx: &mut Context<Self>) {
        self.toast_with(kind, message, None, cx);
    }

    pub fn toast_with(&mut self, kind: ToastKind, message: impl Into<String>, action: Option<ToastAction>, cx: &mut Context<Self>) {
        let message = message.into();
        // Repeated saves replace the previous "Saved" toast instead of stacking.
        self.toasts.retain(|t| !(t.message == message && t.action == action));
        let id = self.next_toast;
        self.next_toast += 1;
        if kind == ToastKind::Success {
            crate::sound::play(crate::sound::Sound::Applied);
        }
        self.toasts.push(Toast { id, kind, message, action });
        if self.toasts.len() > 4 {
            self.toasts.remove(0);
        }
        cx.notify();
        let secs = if kind == ToastKind::Error || action.is_some() { 8 } else { 4 };
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(Duration::from_secs(secs)).await;
            this.update(cx, |ws, cx| {
                ws.toasts.retain(|t| t.id != id);
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    pub fn welcome_done(&mut self, mode: Mode, cx: &mut Context<Self>) {
        self.settings.welcomed = true;
        self.settings.mode = mode;
        self.page = self.home_page();
        self.settings.save();
        cx.notify();
    }

    pub fn dismiss_toast(&mut self, id: u64, cx: &mut Context<Self>) {
        self.toasts.retain(|t| t.id != id);
        cx.notify();
    }
}

/// Label shared by Quick Settings saves, so a burst of edits shares one backup.
const QUICK_LABEL: &str = "Before Quick Settings changes";

/// Patch states for an exe, cached by size and modification time so the
/// multi-megabyte scan only runs when the exe actually changed.
fn scan_exe(def: &GameDef, path: &std::path::Path) -> ExeInfo {
    use std::sync::Mutex;
    type Key = (PathBuf, u64, Option<std::time::SystemTime>);
    static CACHE: Mutex<Vec<(Key, ExeInfo)>> = Mutex::new(Vec::new());
    let meta = std::fs::metadata(path).ok();
    let key: Key = (
        path.to_path_buf(),
        meta.as_ref().map_or(0, |m| m.len()),
        meta.and_then(|m| m.modified().ok()),
    );
    if let Some((_, info)) = CACHE.lock().unwrap_or_else(|e| e.into_inner()).iter().find(|(k, _)| *k == key) {
        return info.clone();
    }
    let bytes = patches::read_exe(path).unwrap_or_default();
    let info = ExeInfo {
        size: bytes.len() as u64,
        patch_states: def.patches.iter().map(|p| (p.id, p.state(&bytes))).collect(),
    };
    let mut cache = CACHE.lock().unwrap_or_else(|e| e.into_inner());
    cache.retain(|(k, _)| k.0 != key.0);
    cache.push((key, info.clone()));
    info
}

/// Work for one setup step.
enum Job {
    Done(String),
    Background(Box<dyn FnOnce() -> Result<String> + Send>),
}

/// Keeps the newest `max` config-only snapshots. Snapshots holding anything
/// else (the game exe, mod manager files) and the original-settings snapshot
/// are never pruned automatically.
fn prune_backups(game_id: &str, max: Option<usize>) {
    let max = max.unwrap_or(30);
    let config_only = |b: &Backup| {
        b.files
            .iter()
            .all(|f| f.original.extension().is_some_and(|e| e.eq_ignore_ascii_case("ini")))
    };
    let prunable = |b: &Backup| config_only(b) && b.label != backup::ORIGINAL_LABEL;
    for old in backup::list(game_id).into_iter().filter(prunable).skip(max) {
        let _ = backup::delete(&old);
    }
}
