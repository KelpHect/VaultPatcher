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

#[derive(Clone, Debug)]
pub struct Toast {
    pub id: u64,
    pub kind: ToastKind,
    pub message: String,
}

#[derive(Clone, Debug, Default)]
pub struct ExeInfo {
    pub size: u64,
    pub patch_states: HashMap<&'static str, PatchState>,
}

pub struct GameState {
    pub def: &'static GameDef,
    pub install: Option<Install>,
    pub config_dir: Option<PathBuf>,
    pub config: ConfigSet,
    pub pending: BTreeMap<&'static str, Value>,
    pub exe: Option<ExeInfo>,
    pub sdk: SdkStatus,
    pub mods: Vec<ModEntry>,
    pub backups: Vec<Backup>,
    /// Components ticked on the setup page.
    pub setup_selected: BTreeSet<&'static str>,
}

impl GameState {
    fn new(def: &'static GameDef) -> Self {
        Self {
            def,
            install: None,
            config_dir: None,
            config: ConfigSet::default(),
            pending: BTreeMap::new(),
            exe: None,
            sdk: SdkStatus::NotInstalled,
            mods: Vec::new(),
            backups: Vec::new(),
            setup_selected: def.setup.iter().filter(|c| c.recommended).map(|c| c.id).collect(),
        }
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
    pub preview: Option<(&'static str, usize)>,
    /// A running (or finished) comparison capture.
    pub capture: Option<std::sync::Arc<std::sync::Mutex<crate::compare::CaptureProgress>>>,
    pub capture_save: Option<String>,
    pub capture_settle: u32,
    next_toast: u64,
    /// Bumped on every Simple-mode edit; a pending auto-apply only runs if
    /// no newer edit arrived in the meantime.
    autoapply_generation: u64,
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
            next_toast: 0,
            autoapply_generation: 0,
        };
        for i in 0..ws.games.len() {
            ws.refresh_game(i);
        }
        ws.fetch_comparison_packs(cx);
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
        if missing.is_empty() {
            return;
        }
        let task = cx.background_executor().spawn(async move {
            missing.iter().filter(|id| crate::compare::download_pack(id).is_ok()).count()
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
        self.page = page;
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

    pub fn set_mode(&mut self, mode: Mode, cx: &mut Context<Self>) {
        if self.settings.mode == mode {
            return;
        }
        self.settings.mode = mode;
        self.settings.save();
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

        game.exe = game.exe_path().filter(|p| p.is_file()).map(|path| {
            let bytes = patches::read_exe(&path).unwrap_or_default();
            ExeInfo {
                size: bytes.len() as u64,
                patch_states: def.patches.iter().map(|p| (p.id, p.state(&bytes))).collect(),
            }
        });

        match (def.mods, &game.install) {
            (Some(support), Some(install)) => {
                game.sdk = mods::sdk_status(support, &install.root);
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

    pub fn stage_preset(&mut self, preset: &Preset, cx: &mut Context<Self>) {
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
            format!("{} staged — {count} pending change(s). Apply to write them.", preset.name),
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
        match self.try_apply_pending() {
            Ok(n) => {
                let id = self.game().def.id;
                self.game_mut().backups = backup::list(id);
                self.toast(ToastKind::Success, format!("Applied {n} change(s). Backup saved."), cx);
            }
            Err(e) => self.toast(ToastKind::Error, format!("Apply failed: {e:#}"), cx),
        }
    }

    fn try_apply_pending(&mut self) -> Result<usize> {
        let pending = std::mem::take(&mut self.game_mut().pending);
        let def = self.game().def;
        let label = format!("Before applying {} tweak(s)", pending.len());
        let result = self.write_configs(&label, |config| {
            for (id, value) in &pending {
                if let Some(tweak) = def.tweak(id) {
                    tweak.write(config, value);
                }
            }
        });
        match result {
            Ok(()) => Ok(pending.len()),
            Err(e) => {
                self.game_mut().pending = pending;
                Err(e)
            }
        }
    }

    /// Edits a copy of the active game's configs with `edit`, snapshots the
    /// files that changed, and writes them.
    fn write_configs(&mut self, label: &str, edit: impl FnOnce(&mut ConfigSet)) -> Result<()> {
        let lock = self.settings.lock_configs_after_apply;
        let max_backups = self.settings.max_backups;
        let game = self.game_mut();
        if game.config_dir.is_none() || !game.config_found() {
            return Err(anyhow!(
                "config files not found — launch the game once or set the folder in App Settings"
            ));
        }
        let mut config = game.config.clone();
        edit(&mut config);
        let dirty = config.dirty_paths();
        if dirty.is_empty() {
            return Ok(());
        }
        backup::create(game.def.id, label, &dirty).context("creating backup")?;
        let written = config.save_dirty()?;
        if lock {
            for path in &written {
                backup::set_readonly(path, true)?;
            }
        }
        game.config = config;
        prune_backups(game.def.id, max_backups);
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
        self.autoapply_generation += 1;
        let generation = self.autoapply_generation;
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(Duration::from_millis(700)).await;
            this.update(cx, |ws, cx| {
                if ws.autoapply_generation != generation || ws.game().pending.is_empty() {
                    return;
                }
                match ws.try_apply_pending() {
                    Ok(_) => {
                        let id = ws.game().def.id;
                        ws.game_mut().backups = backup::list(id);
                        ws.toast(ToastKind::Success, "Saved", cx);
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
        setup::status(component, self.game(), &self.launch_args())
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

    pub fn open_preview(&mut self, tweak: &'static str, index: usize, cx: &mut Context<Self>) {
        self.preview = Some((tweak, index));
        cx.notify();
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
        let mut ids: Vec<&'static str> = game
            .def
            .setup
            .iter()
            .filter(|c| {
                let status = self.component_status(c);
                if uninstall {
                    matches!(status, Status::Active(_) | Status::Partial)
                } else {
                    game.setup_selected.contains(c.id)
                        && !status.is_active()
                        && !matches!(status, Status::Blocked(_))
                }
            })
            .map(|c| c.id)
            .collect();
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
                        ws.skip_dependents(id, cx);
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

    fn skip_dependents(&mut self, failed: &str, cx: &mut Context<Self>) {
        let def = self.game().def;
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
        self.active = game_index;
        let skipped = self.setup_run.as_ref().is_some_and(|run| {
            run.steps.iter().any(|(s, st)| *s == id && matches!(st, StepState::Failed(_)))
        });
        if skipped {
            return None;
        }
        let def = self.game().def;
        let component = def.component(id)?;
        Some(self.component_job(component, uninstall))
    }

    fn component_job(&mut self, component: &'static setup::Component, uninstall: bool) -> Result<Job> {
        let def = self.game().def;
        let root = self.game().install.as_ref().map(|i| i.root.clone());
        let need_root = || root.clone().context("game install not found");
        let (game_id, comp_id) = (def.id, component.id);
        Ok(match (component.kind, uninstall) {
            (ComponentKind::Settings { values, display }, false) => {
                let mode = crate::core::display::primary();
                self.write_configs(&format!("Before {}", component.name), |config| {
                    for (id, v) in values {
                        if let Some(t) = def.tweak(id) {
                            t.write(config, &v.to_value());
                        }
                    }
                    if let (Some(hook), Some(mode)) = (display, mode) {
                        hook(config, mode);
                    }
                })?;
                Job::Done(match (display, mode) {
                    (Some(_), Some(m)) => format!("Tuned for {}×{} @ {} Hz", m.width, m.height, m.refresh_hz),
                    _ => "Applied".into(),
                })
            }
            (ComponentKind::Settings { values, .. }, true) => {
                self.write_configs(&format!("Before removing {}", component.name), |config| {
                    for (id, _) in values {
                        if let Some(t) = def.tweak(id) {
                            t.write(config, &t.default.to_value());
                        }
                    }
                })?;
                Job::Done("Back to game defaults".into())
            }
            (ComponentKind::ExePatch(patch), uninstall) => {
                let p = def.patches.iter().find(|p| p.id == patch).context("unknown patch")?;
                if uninstall && !p.revertible {
                    return Ok(Job::Done("Kept (the game ships with it)".into()));
                }
                let path = self.game().exe_path().context("game install not found")?;
                let mut bytes = patches::read_exe(&path)?;
                p.set(&mut bytes, !uninstall)?;
                backup::create(game_id, &format!("Before {}", p.name), std::slice::from_ref(&path))?;
                backup::clear_readonly(&path);
                std::fs::write(&path, bytes)
                    .with_context(|| format!("writing {} (is the game running?)", path.display()))?;
                Job::Done(if uninstall { "Reverted" } else { "Patched" }.into())
            }
            (ComponentKind::LaunchArg(arg), remove) => {
                let mut args = self.launch_args();
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

    pub fn restore_backup(&mut self, index: usize, cx: &mut Context<Self>) {
        let Some(b) = self.game().backups.get(index).cloned() else {
            return;
        };
        match backup::restore(&b) {
            Ok(()) => {
                self.refresh_active(cx);
                self.toast(ToastKind::Success, format!("Restored \"{}\"", b.label), cx);
            }
            Err(e) => self.toast(ToastKind::Error, format!("Restore failed: {e:#}"), cx),
        }
    }

    pub fn delete_backup(&mut self, index: usize, cx: &mut Context<Self>) {
        let Some(b) = self.game().backups.get(index).cloned() else {
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

    pub fn set_mod_enabled(&mut self, index: usize, enabled: bool, cx: &mut Context<Self>) {
        let game = self.game();
        let (Some(support), Some(install)) = (game.def.mods, game.install.clone()) else {
            return;
        };
        let Some(entry) = game.mods.get(index).cloned() else {
            return;
        };
        if let Err(e) = mods::set_enabled(support, &install.root, &entry, enabled) {
            self.toast(ToastKind::Error, format!("{e:#}"), cx);
        }
        self.refresh_active(cx);
    }

    pub fn remove_mod(&mut self, index: usize, cx: &mut Context<Self>) {
        let Some(entry) = self.game().mods.get(index).cloned() else {
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
        let def = self.game().def;
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
        let id = self.next_toast;
        self.next_toast += 1;
        self.toasts.push(Toast {
            id,
            kind,
            message: message.into(),
        });
        if self.toasts.len() > 4 {
            self.toasts.remove(0);
        }
        cx.notify();
        let secs = if kind == ToastKind::Error { 8 } else { 4 };
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

    pub fn dismiss_toast(&mut self, id: u64, cx: &mut Context<Self>) {
        self.toasts.retain(|t| t.id != id);
        cx.notify();
    }
}

/// Keeps the newest `max` config-only snapshots. Snapshots holding anything
/// else (the game exe, mod manager files) are never pruned automatically.
/// Work for one setup step.
enum Job {
    Done(String),
    Background(Box<dyn FnOnce() -> Result<String> + Send>),
}

fn prune_backups(game_id: &str, max: Option<usize>) {
    let max = max.unwrap_or(30);
    let config_only = |b: &Backup| {
        b.files
            .iter()
            .all(|f| f.original.extension().is_some_and(|e| e.eq_ignore_ascii_case("ini")))
    };
    for old in backup::list(game_id).into_iter().filter(config_only).skip(max) {
        let _ = backup::delete(&old);
    }
}
