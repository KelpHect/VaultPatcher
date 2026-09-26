//! The main window: title bar (game switcher, mode switch, window controls),
//! navigation rail, the active page, the Apply bar, status bar, toasts, and
//! the overlays (comparison viewer, change review, first-run welcome).

use gpui::{
    AnyElement, Context, Corner, ElementId, Entity, ExternalPaths, FontWeight, IntoElement, ObjectFit, ParentElement,
    Render, SharedString, Styled, Subscription, Window, WindowControlArea, div, img, prelude::*, px,
};
use gpui_component::button::{Button, ButtonVariants as _};
use gpui_component::input::{InputEvent, InputState};
use gpui_component::menu::{DropdownMenu as _, PopupMenuItem};

use crate::games::{Mode, NavItem, PageKind};
use crate::pages;
use crate::theme::{self, Icon};
use crate::ui::{self, Variant};
use crate::workspace::{LaunchMode, ToastAction, ToastKind, Workspace};

pub struct Shell {
    ws: Entity<Workspace>,
    focus: gpui::FocusHandle,
    _subs: Vec<Subscription>,
}

impl Shell {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let ws = cx.new(Workspace::new);
        let search = cx.new(|cx| InputState::new(window, cx).placeholder("Search settings  (Ctrl+F)"));
        let profile = cx.new(|cx| InputState::new(window, cx).placeholder("Profile name"));
        let search_ws = ws.clone();
        let subs = vec![
            cx.observe(&ws, |_, _, cx| cx.notify()),
            cx.subscribe(&search, move |_, input, ev: &InputEvent, cx| {
                if matches!(ev, InputEvent::Change) {
                    let text = input.read(cx).value().to_string();
                    search_ws.update(cx, |ws, cx| ws.set_search(text, cx));
                }
            }),
        ];
        ws.update(cx, |ws, _| {
            ws.search_input = Some(search);
            ws.profile_input = Some(profile);
        });
        Self { ws, focus: cx.focus_handle(), _subs: subs }
    }

    /// App-wide keys. Esc closes overlays, ←/→ flip the viewer's right side,
    /// Ctrl+S applies, Ctrl+F jumps to search.
    fn on_key(&mut self, ev: &gpui::KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let key = ev.keystroke.key.as_str();
        let ctrl = ev.keystroke.modifiers.control;
        if ctrl && key == "f" {
            let input = self.ws.read(cx).search_input.clone();
            let page = self.ws.read(cx).page;
            if !matches!(page, PageKind::Tweaks(_) | PageKind::Quick) {
                let first = self.ws.read(cx).game().def.nav_items(self.ws.read(cx).mode()).find(|n| matches!(n.kind, PageKind::Tweaks(_) | PageKind::Quick)).map(|n| n.kind);
                if let Some(kind) = first {
                    self.ws.update(cx, |ws, cx| ws.navigate(kind, cx));
                }
            }
            if let Some(input) = input {
                input.update(cx, |i, cx| i.focus(window, cx));
            }
            return;
        }
        self.ws.update(cx, |ws, cx| {
            if key == "escape" && ws.review_open {
                ws.set_review(false, cx);
                return;
            }
            if let Some(preview) = ws.preview {
                let count = ws
                    .game()
                    .def
                    .tweak(preview.tweak)
                    .map(|t| crate::compare::images(ws.game().def.id, t).len())
                    .unwrap_or(0)
                    .max(1);
                match key {
                    "escape" => ws.close_preview(cx),
                    "right" => ws.set_preview(|p| p.right = (p.right + 1) % count, cx),
                    "left" => ws.set_preview(|p| p.right = (p.right + count - 1) % count, cx),
                    _ => {}
                }
                return;
            }
            if ctrl && key == "s" && ws.pending_count() > 0 && ws.mode() == Mode::Advanced {
                ws.apply_pending(cx);
            }
        });
    }

    // ---- title bar ---------------------------------------------------------------

    fn title_bar(&self, window: &Window, cx: &Context<Self>) -> impl IntoElement {
        let state = self.ws.read(cx);
        let maximized = window.is_maximized();
        let update = state.updates.app.clone();
        div()
            .h(px(40.))
            .flex_none()
            .flex()
            .items_center()
            .bg(theme::bg_deep())
            .border_b_1()
            .border_color(theme::line())
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(10.))
                    .pl(px(14.))
                    .pr(px(10.))
                    .h_full()
                    .window_control_area(WindowControlArea::Drag)
                    .child(app_mark(22.))
                    .child(
                        div()
                            .font_family(theme::FONT_TITLE)
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_size(px(17.))
                            .text_color(theme::text())
                            .child("Vault Patcher"),
                    ),
            )
            .child(div().w(px(1.)).h(px(18.)).bg(theme::line()))
            .child(div().px(px(6.)).child(self.game_switcher(cx)))
            .child(div().id("drag").flex_1().h_full().window_control_area(WindowControlArea::Drag))
            .when_some(update, |d, (tag, page)| {
                d.child(
                    div()
                        .id("update")
                        .mr(px(10.))
                        .flex()
                        .items_center()
                        .gap(px(6.))
                        .h(px(26.))
                        .px(px(10.))
                        .rounded(px(theme::RADIUS))
                        .bg(theme::with_alpha(theme::success(), 0.15))
                        .text_color(theme::success())
                        .text_size(px(12.5))
                        .font_weight(FontWeight::SEMIBOLD)
                        .cursor_pointer()
                        .child(ui::icon(Icon::Download).size(px(14.)).text_color(theme::success()))
                        .child(format!("Update {tag}"))
                        .tooltip(ui::tip("A newer Vault Patcher is available. Opens the download page."))
                        .on_click(move |_, _, cx| cx.open_url(&page)),
                )
            })
            .child(self.mode_toggle(cx))
            .child(chrome_button("min", Icon::Minimize, WindowControlArea::Min, false))
            .child(chrome_button("max", if maximized { Icon::Restore } else { Icon::Maximize }, WindowControlArea::Max, false))
            .child(chrome_button("close", Icon::Close, WindowControlArea::Close, true))
    }

    /// Current game (its own icon when found) with a menu to switch.
    fn game_switcher(&self, cx: &Context<Self>) -> impl IntoElement {
        let state = self.ws.read(cx);
        let game = state.game();
        type Entry = (usize, &'static str, &'static str, Option<std::path::PathBuf>, &'static str, bool);
        let games: Vec<Entry> = state
            .games
            .iter()
            .enumerate()
            .map(|(i, g)| (i, g.def.name, g.def.short, g.art.icon.clone(), status_text(g), i == state.active))
            .collect();
        let ws = self.ws.clone();
        Button::new("game-switcher")
            .ghost()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .child(game_icon(game.art.icon.clone(), game.def.short, 20.))
                    .child(div().text_size(px(13.5)).font_weight(FontWeight::SEMIBOLD).text_color(theme::text()).child(game.def.name))
                    .child(div().size(px(7.)).rounded_full().bg(status_color(game)))
                    .child(ui::icon(Icon::ChevronDown).size(px(14.))),
            )
            .dropdown_menu(move |mut menu, _, _| {
                for (i, name, short, icon, status, active) in games.clone() {
                    let ws = ws.clone();
                    menu = menu.item(
                        PopupMenuItem::element(move |_, _| {
                            div()
                                .flex()
                                .items_center()
                                .gap(px(10.))
                                .py(px(3.))
                                .child(game_icon(icon.clone(), short, 28.))
                                .child(
                                    div()
                                        .flex()
                                        .flex_col()
                                        .child(div().text_size(px(13.5)).font_weight(FontWeight::SEMIBOLD).child(name))
                                        .child(div().text_size(px(12.)).text_color(theme::text_dim()).child(status)),
                                )
                        })
                        .checked(active)
                        .on_click(move |_, _, cx| ws.update(cx, |ws, cx| ws.select_game(i, cx))),
                    );
                }
                menu.min_w(px(260.))
            })
    }

    /// Simple / Advanced switch in the title bar.
    fn mode_toggle(&self, cx: &Context<Self>) -> impl IntoElement {
        let mode = self.ws.read(cx).mode();
        let (wrap, segments) = ui::segmented(vec![
            ("mode-simple".into(), "Simple".into(), mode == Mode::Simple),
            ("mode-advanced".into(), "Advanced".into(), mode == Mode::Advanced),
        ]);
        let mut wrap = wrap.mr(px(8.));
        for (seg, m) in segments.into_iter().zip([Mode::Simple, Mode::Advanced]) {
            let ws = self.ws.clone();
            let tip = match m {
                Mode::Simple => "One-click setup and a few friendly settings, saved as you go",
                Mode::Advanced => "Every setting, presets, exe patches and the mod manager",
            };
            wrap = wrap.child(seg.tooltip(ui::tip(tip)).on_click(move |_, window, cx| {
                // Simple mode saves instantly, so changes still waiting
                // from Advanced must be dealt with first.
                let pending = ws.read(cx).pending_count();
                if m != Mode::Simple || pending == 0 {
                    ws.update(cx, |ws, cx| ws.set_mode(m, cx));
                    return;
                }
                let answer = window.prompt(
                    gpui::PromptLevel::Info,
                    &format!("You have {pending} change(s) waiting to be applied."),
                    Some("Simple mode saves changes immediately. Apply the waiting changes now, or discard them?"),
                    &["Apply and switch", "Discard and switch", "Cancel"],
                    cx,
                );
                let ws = ws.clone();
                cx.spawn(async move |cx| {
                    let choice = answer.await.ok();
                    cx.update(|cx| {
                        ws.update(cx, |ws, cx| match choice {
                            Some(0) => {
                                ws.apply_pending(cx);
                                if ws.pending_count() == 0 {
                                    ws.set_mode(m, cx);
                                }
                            }
                            Some(1) => {
                                ws.discard_pending(cx);
                                ws.set_mode(m, cx);
                            }
                            _ => {}
                        })
                    })
                    .ok();
                })
                .detach();
            }));
        }
        wrap
    }

    // ---- navigation rail ---------------------------------------------------------

    fn rail(&self, cx: &Context<Self>) -> impl IntoElement {
        let state = self.ws.read(cx);
        let game = state.game();
        let mut nav = div().flex().flex_col().pb(px(12.));
        for group in game.def.nav_for(state.mode()) {
            nav = nav.child(ui::label(group.title).px(px(18.)).pt(px(16.)).pb(px(6.)));
            for item in group.items {
                nav = nav.child(self.nav_entry(item, cx));
            }
        }

        let play_ws = self.ws.clone();
        let can_play = game.install.is_some();
        let running = game.is_running();
        let short = game.def.short;
        // This game ships a separate launcher (BL2/TPS): the menu offers ways
        // around it. "Through the launcher" additionally needs the file there.
        let launcher_known = game.def.launcher.is_some();
        let launcher_exists = game
            .def
            .launcher
            .and_then(|l| game.install.as_ref().map(|i| i.root.join(l.exe)))
            .is_some_and(|p| p.is_file());
        let menu_ws = self.ws.clone();
        let reapply_ws = self.ws.clone();
        let current_mode = state.launch_mode();
        // Clicking an entry makes it the Play button's way of launching (the
        // checkmark) and starts the game that way right now, if it can.
        let launch_item = move |label: String, detail: &'static str, icon: Icon, mode: LaunchMode| {
            let ws = menu_ws.clone();
            PopupMenuItem::element(move |_, _| {
                div()
                    .flex()
                    .items_center()
                    .gap(px(10.))
                    .py(px(2.))
                    .child(ui::icon(icon).size(px(15.)).text_color(theme::text_dim()))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .child(div().text_size(px(13.)).font_weight(FontWeight::SEMIBOLD).child(label.clone()))
                            .child(div().text_size(px(11.5)).text_color(theme::text_dim()).child(detail)),
                    )
            })
            .checked(mode == current_mode)
            .on_click(move |_, _, cx| {
                ws.update(cx, |ws, cx| {
                    ws.set_launch_mode(mode, cx);
                    if can_play && !running {
                        ws.launch(mode, cx);
                    }
                })
            })
        };
        let play = div().p(px(10.)).border_t_1().border_color(theme::line()).child(
            div()
                .flex()
                .child(
                    ui::button(
                        "rail-play",
                        if running { "Running".to_string() } else { format!("Play {short}") },
                        Some(Icon::Play),
                        Variant::Primary,
                    )
                    .flex_1()
                    .min_w_0()
                    .h(px(36.))
                    .rounded_tr(px(0.))
                    .rounded_br(px(0.))
                    .when(!can_play || running, |b| b.opacity(0.45))
                    .tooltip(ui::tip(if can_play {
                        match current_mode {
                            LaunchMode::Normal => "Start the game with your launch options",
                            LaunchMode::Direct => "Start the game exe directly — skipping the launcher",
                            LaunchMode::Launcher => "Start the game through its own launcher",
                        }
                    } else {
                        "Game install not found"
                    }))
                    .on_click(move |_, _, cx| {
                        if can_play && !running {
                            play_ws.update(cx, |ws, cx| ws.launch_default(cx))
                        }
                    }),
                )
                .child(
                    Button::new("rail-play-menu")
                        .primary()
                        .h(px(36.))
                        .w(px(30.))
                        .flex_none()
                        .rounded_tl(px(0.))
                        .rounded_bl(px(0.))
                        .border_l_1()
                        .border_color(theme::accent_pressed())
                        .tooltip("More ways to start the game")
                        .child(ui::icon(Icon::ChevronDown).size(px(15.)).text_color(theme::accent_ink()))
                        .dropdown_menu_with_anchor(Corner::BottomLeft, move |mut menu, _, _| {
                            menu = menu.item(launch_item(
                                format!("Play {short}"),
                                "With your launch options",
                                Icon::Play,
                                LaunchMode::Normal,
                            ));
                            if launcher_known {
                                menu = menu.item(launch_item(
                                    "Skip the launcher".into(),
                                    "The game exe directly — nothing rewrites your settings",
                                    Icon::Rocket,
                                    LaunchMode::Direct,
                                ));
                            }
                            if launcher_exists {
                                menu = menu.item(launch_item(
                                    "Through the game launcher".into(),
                                    "The game's own menu — it may re-apply its video settings",
                                    Icon::Game,
                                    LaunchMode::Launcher,
                                ));
                            }
                            let reapply_ws = reapply_ws.clone();
                            menu.separator().item(
                                PopupMenuItem::element(move |_, _| {
                                    div()
                                        .flex()
                                        .items_center()
                                        .gap(px(10.))
                                        .py(px(2.))
                                        .child(ui::icon(Icon::Refresh).size(px(15.)).text_color(theme::text_dim()))
                                        .child(
                                            div()
                                                .flex()
                                                .flex_col()
                                                .child(div().text_size(px(13.)).font_weight(FontWeight::SEMIBOLD).child("Re-apply settings & upgrades"))
                                                .child(
                                                    div()
                                                        .text_size(px(11.5))
                                                        .text_color(theme::text_dim())
                                                        .child("The game or its launcher reset things? Put them all back"),
                                                ),
                                        )
                                })
                                .on_click(move |_, _, cx| reapply_ws.update(cx, |ws, cx| ws.reapply_all(cx))),
                            )
                            .min_w(px(300.))
                        }),
                ),
        );

        div()
            .w(px(216.))
            .flex_none()
            .h_full()
            .flex()
            .flex_col()
            .bg(theme::bg_deep())
            .border_r_1()
            .border_color(theme::line())
            .child(div().id("nav-scroll").flex_1().min_h_0().overflow_y_scroll().child(nav))
            .child(play)
    }

    fn nav_entry(&self, item: &'static NavItem, cx: &Context<Self>) -> impl IntoElement {
        let state = self.ws.read(cx);
        let game = state.game();
        let active = state.page == item.kind;
        let waiting = game
            .pending
            .keys()
            .filter(|id| game.def.tweak(id).is_some_and(|t| item.categories.contains(&t.category)))
            .count();
        let ws = self.ws.clone();
        let kind = item.kind;
        div()
            .id(SharedString::from(format!("nav-{}", item.title)))
            .relative()
            .mx(px(8.))
            .flex()
            .items_center()
            .gap(px(10.))
            .h(px(32.))
            .px(px(10.))
            .rounded(px(theme::RADIUS))
            .bg(if active { theme::selected() } else { gpui::transparent_black().into() })
            .when(!active, |d| d.hover(|s| s.bg(theme::panel_hi())))
            .cursor_pointer()
            .when(active, |d| {
                d.child(div().absolute().left_0().top(px(8.)).bottom(px(8.)).w(px(3.)).rounded_full().bg(theme::accent()))
            })
            .child(ui::icon(item.icon).text_color(if active { theme::accent() } else { theme::text_muted() }))
            .child(
                div()
                    .flex_1()
                    .text_size(px(13.5))
                    .font_weight(if active { FontWeight::SEMIBOLD } else { FontWeight::NORMAL })
                    .text_color(if active { theme::text() } else { theme::text_muted() })
                    .child(item.title),
            )
            .when(waiting > 0, |d| {
                d.child(
                    div()
                        .px(px(6.))
                        .h(px(18.))
                        .flex()
                        .items_center()
                        .rounded_full()
                        .bg(theme::accent())
                        .text_color(theme::accent_ink())
                        .font_weight(FontWeight::BOLD)
                        .text_size(px(11.))
                        .child(waiting.to_string()),
                )
            })
            .on_click(move |_, window, cx| {
                // A new page starts with a clear search.
                if let Some(input) = ws.read(cx).search_input.clone() {
                    input.update(cx, |i, cx| i.set_value("", window, cx));
                }
                ws.update(cx, |ws, cx| {
                    ws.set_search(String::new(), cx);
                    ws.navigate(kind, cx)
                })
            })
    }

    // ---- bottom bars --------------------------------------------------------------

    fn pending_bar(&self, cx: &Context<Self>) -> Option<AnyElement> {
        let state = self.ws.read(cx);
        let game = state.game();
        // Simple mode saves automatically, so there's nothing to confirm.
        if game.pending.is_empty() || state.mode() == Mode::Simple {
            return None;
        }
        let names: Vec<&str> = game.pending.keys().filter_map(|id| game.def.tweak(id).map(|t| t.label)).take(4).collect();
        let more = game.pending.len().saturating_sub(names.len());
        let review_ws = self.ws.clone();
        let discard_ws = self.ws.clone();
        let apply_ws = self.ws.clone();
        Some(
            div()
                .flex_none()
                .h(px(52.))
                .flex()
                .items_center()
                .gap(px(12.))
                .px(px(16.))
                .bg(theme::bg_deep())
                .border_t_1()
                .border_color(theme::line())
                .child(div().w(px(3.)).h(px(28.)).rounded_full().bg(theme::accent()))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .child(
                            div()
                                .text_size(px(13.5))
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(theme::text())
                                .child(format!("{} change(s) waiting", game.pending.len())),
                        )
                        .child(div().text_size(px(12.)).text_color(theme::text_dim()).truncate().child(format!(
                            "{}{}",
                            names.join(" · "),
                            if more > 0 { format!(" · +{more} more") } else { String::new() }
                        ))),
                )
                .child(
                    ui::button("review", "Review", Some(Icon::Search), Variant::Ghost)
                        .tooltip(ui::tip("See every change before it's written"))
                        .on_click(move |_, _, cx| review_ws.update(cx, |ws, cx| ws.set_review(true, cx))),
                )
                .child(
                    ui::button("discard", "Discard", None, Variant::Secondary)
                        .on_click(move |_, _, cx| discard_ws.update(cx, |ws, cx| ws.discard_pending(cx))),
                )
                .child(
                    ui::button("apply", "Apply", Some(Icon::Check), Variant::Primary)
                        .tooltip(ui::tip("Write the changes (Ctrl+S). A backup is made first."))
                        .on_click(move |_, _, cx| apply_ws.update(cx, |ws, cx| ws.apply_pending(cx))),
                )
                .into_any_element(),
        )
    }

    fn status_bar(&self, cx: &Context<Self>) -> impl IntoElement {
        use crate::mods::SdkStatus;
        let state = self.ws.read(cx);
        let game = state.game();
        let where_ = game
            .install
            .as_ref()
            .map(|i| format!("{} · {}", i.store.label(), i.root.display()))
            .unwrap_or_else(|| "Game not found".into());
        let sdk = match &game.sdk {
            SdkStatus::Installed(v) => format!("SDK {v}"),
            SdkStatus::Detected => "SDK installed".into(),
            SdkStatus::Legacy => "Legacy SDK".into(),
            SdkStatus::NotInstalled => "No SDK".into(),
        };
        let active = game.def.setup.iter().filter(|c| state.component_status(c).is_active()).count();
        let muted = state.settings.sound_muted;
        let music = state.settings.music;
        let sounds = crate::sound::available();
        let mute_ws = self.ws.clone();
        let music_ws = self.ws.clone();

        let item = |text: String| div().flex_none().text_size(px(12.)).text_color(theme::text_dim()).child(text);
        let sep = || div().w(px(1.)).h(px(12.)).bg(theme::line());
        let toggle = |id: &'static str, glyph: Icon, on: bool, tip: &'static str| {
            div()
                .id(id)
                .flex()
                .items_center()
                .justify_center()
                .size(px(22.))
                .rounded(px(theme::RADIUS))
                .cursor_pointer()
                .hover(|s| s.bg(theme::panel_hi()))
                .tooltip(ui::tip(tip))
                .child(ui::icon(glyph).size(px(13.)).text_color(if on { theme::accent() } else { theme::text_dim() }))
        };

        div()
            .h(px(28.))
            .flex_none()
            .flex()
            .items_center()
            .gap(px(10.))
            .px(px(12.))
            .bg(theme::bg_deep())
            .border_t_1()
            .border_color(theme::line())
            .child(div().size(px(7.)).flex_none().rounded_full().bg(status_color(game)))
            .child(div().flex_none().text_size(px(12.)).font_weight(FontWeight::SEMIBOLD).text_color(theme::text_muted()).child(game.def.name))
            .child(div().flex_1().min_w_0().truncate().text_size(px(12.)).text_color(theme::text_dim()).child(where_))
            .when_some(state.busy.clone(), |d, b| {
                d.child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(6.))
                        .child(ui::icon(Icon::Loader).size(px(13.)).text_color(theme::accent()))
                        .child(item(b).text_color(theme::accent())),
                )
                .child(sep())
            })
            .child(item(sdk))
            .child(sep())
            .child(item(format!("{active}/{} upgrades", game.def.setup.len())))
            .child(sep())
            .child(
                toggle("sb-music", Icon::Music, music && !muted && sounds, "Menu music")
                    .when(!sounds, |d| d.opacity(0.4))
                    .on_click(move |_, _, cx| music_ws.update(cx, |ws, cx| ws.set_music(!music, cx))),
            )
            .child(
                toggle(
                    "sb-mute",
                    if muted { Icon::Mute } else { Icon::Volume },
                    !muted && sounds,
                    if sounds { "Button sounds (from the game's launcher)" } else { "No game sounds found" },
                )
                .when(!sounds, |d| d.opacity(0.4))
                .on_click(move |_, _, cx| mute_ws.update(cx, |ws, cx| ws.set_muted(!muted, cx))),
            )
            .child(sep())
            .child(item(format!("v{}", env!("CARGO_PKG_VERSION"))))
    }

    fn toasts(&self, bottom: f32, cx: &Context<Self>) -> impl IntoElement {
        let state = self.ws.read(cx);
        let mut stack = div().absolute().bottom(px(bottom)).right(px(16.)).w(px(360.)).flex().flex_col().gap(px(8.));
        for toast in &state.toasts {
            let (color, icon) = match toast.kind {
                ToastKind::Info => (theme::echo(), Icon::Info),
                ToastKind::Success => (theme::success(), Icon::CheckCircle),
                ToastKind::Error => (theme::danger(), Icon::Alert),
            };
            let ws = self.ws.clone();
            let action_ws = self.ws.clone();
            let id = toast.id;
            stack = stack.child(
                div()
                    .id(ElementId::Integer(id))
                    .flex()
                    .items_start()
                    .gap(px(10.))
                    .p(px(12.))
                    .bg(theme::panel())
                    .border_1()
                    .border_color(theme::line())
                    .rounded(px(theme::RADIUS_LG))
                    .shadow(theme::shadow())
                    .occlude()
                    .child(ui::icon(icon).text_color(color).mt(px(1.)))
                    .child(div().flex_1().min_w_0().text_size(px(13.)).line_height(px(18.)).text_color(theme::text()).child(toast.message.clone()))
                    .when_some(toast.action, |d, action| {
                        d.child(
                            div()
                                .id(ElementId::Name(format!("toast-action-{id}").into()))
                                .flex_none()
                                .px(px(8.))
                                .h(px(22.))
                                .flex()
                                .items_center()
                                .rounded(px(theme::RADIUS))
                                .text_size(px(12.5))
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(theme::accent())
                                .cursor_pointer()
                                .hover(|s| s.bg(theme::panel_hi()))
                                .child(match action {
                                    ToastAction::Undo => "Undo",
                                })
                                .on_click(move |_, _, cx| {
                                    cx.stop_propagation();
                                    action_ws.update(cx, |ws, cx| {
                                        ws.dismiss_toast(id, cx);
                                        match action {
                                            ToastAction::Undo => ws.undo_last(cx),
                                        }
                                    })
                                }),
                        )
                    })
                    .child(
                        ui::icon_button(ElementId::Name(format!("toast-close-{id}").into()), Icon::Close, theme::text_dim())
                            .size(px(20.))
                            .on_click(move |_, _, cx| ws.update(cx, |ws, cx| ws.dismiss_toast(id, cx))),
                    ),
            );
        }
        stack
    }

    // ---- overlays -----------------------------------------------------------------

    /// Every waiting change as "old → new", with per-row removal.
    fn review(&self, cx: &Context<Self>) -> Option<AnyElement> {
        let state = self.ws.read(cx);
        if !state.review_open {
            return None;
        }
        let game = state.game();
        let mut rows = div().flex().flex_col();
        for (i, (id, new)) in game.pending.iter().enumerate() {
            let Some(tweak) = game.def.tweak(id) else { continue };
            let old = game.current(tweak).unwrap_or_else(|| tweak.default.to_value());
            let category = game.def.category(tweak.category).map_or("", |c| c.title);
            let ws = self.ws.clone();
            let id = *id;
            rows = rows.when(i > 0, |d| d.child(ui::divider())).child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(12.))
                    .px(px(16.))
                    .py(px(9.))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .flex()
                            .flex_col()
                            .child(ui::title(tweak.label))
                            .child(div().text_size(px(12.)).text_color(theme::text_dim()).child(category)),
                    )
                    .child(div().text_size(px(13.)).text_color(theme::text_dim()).child(old.display(&tweak.control)))
                    .child(ui::icon(Icon::ChevronRight).size(px(14.)).text_color(theme::text_dim()))
                    .child(div().text_size(px(13.)).font_weight(FontWeight::SEMIBOLD).text_color(theme::accent()).child(new.display(&tweak.control)))
                    .child(
                        ui::icon_button(SharedString::from(format!("unstage-{id}")), Icon::Close, theme::text_dim())
                            .tooltip(ui::tip("Drop this change"))
                            .on_click(move |_, _, cx| ws.update(cx, |ws, cx| ws.unstage(id, cx))),
                    ),
            );
        }
        let close_ws = self.ws.clone();
        let backdrop_ws = self.ws.clone();
        let apply_ws = self.ws.clone();
        let count = game.pending.len();
        Some(modal(
            "review-modal",
            560.,
            div()
                .flex()
                .flex_col()
                .child(
                    div()
                        .flex()
                        .items_center()
                        .px(px(16.))
                        .py(px(12.))
                        .child(div().flex_1().child(ui::display("Review changes", 22.)))
                        .child(
                            ui::icon_button("review-close", Icon::Close, theme::text_muted())
                                .on_click(move |_, _, cx| close_ws.update(cx, |ws, cx| ws.set_review(false, cx))),
                        ),
                )
                .child(ui::divider())
                .child(div().id("review-scroll").max_h(px(420.)).overflow_y_scroll().child(rows))
                .child(ui::divider())
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(10.))
                        .px(px(16.))
                        .py(px(12.))
                        .child(ui::body("A backup of the files is made before writing.").flex_1().text_color(theme::text_dim()))
                        .child(
                            ui::button("review-apply", format!("Apply {count}"), Some(Icon::Check), Variant::Primary)
                                .on_click(move |_, _, cx| apply_ws.update(cx, |ws, cx| ws.apply_pending(cx))),
                        ),
                ),
            move |cx| backdrop_ws.update(cx, |ws, cx| ws.set_review(false, cx)),
        ))
    }

    /// First run: what was found, and which mode to start in.
    fn welcome(&self, cx: &Context<Self>) -> Option<AnyElement> {
        let state = self.ws.read(cx);
        if state.settings.welcomed {
            return None;
        }
        let mut found = div().flex().flex_col().gap(px(6.));
        for g in &state.games {
            found = found.child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(10.))
                    .child(game_icon(g.art.icon.clone(), g.def.short, 24.))
                    .child(div().flex_1().text_size(px(13.5)).text_color(theme::text()).child(g.def.name))
                    .child(div().size(px(7.)).rounded_full().bg(status_color(g)))
                    .child(div().text_size(px(12.5)).text_color(theme::text_dim()).child(status_text(g))),
            );
        }
        let mode_card = |id: &'static str, title: &'static str, text: &'static str, icon: Icon, mode: Mode, recommended: bool| {
            let ws = self.ws.clone();
            div()
                .id(id)
                .flex_1()
                .flex_basis(px(0.))
                .min_w_0()
                .flex()
                .flex_col()
                .gap(px(6.))
                .p(px(14.))
                .rounded(px(theme::RADIUS_LG))
                .border_1()
                .border_color(if recommended { theme::accent() } else { theme::line() })
                .bg(if recommended { theme::selected() } else { theme::panel_lo() })
                .cursor_pointer()
                .hover(|s| s.bg(theme::panel_hi()))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(8.))
                        .child(ui::icon(icon).text_color(theme::accent()))
                        .child(ui::title(title))
                        .when(recommended, |d| d.child(ui::badge("Recommended", theme::accent()))),
                )
                .child(ui::body(text))
                .on_click(move |_, _, cx| ws.update(cx, |ws, cx| ws.welcome_done(mode, cx)))
        };
        Some(modal(
            "welcome",
            600.,
            div()
                .flex()
                .flex_col()
                .gap(px(18.))
                .p(px(24.))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(12.))
                        .child(app_mark(48.))
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .flex()
                                .flex_col()
                                .child(ui::display("Welcome to Vault Patcher", 26.))
                                .child(ui::body("Fixes, settings and mods for the Borderlands games. Every change is backed up and can be undone.")),
                        ),
                )
                .child(div().flex().flex_col().gap(px(8.)).child(ui::label("Games on this PC")).child(found))
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(8.))
                        .child(ui::label("How do you want to start?"))
                        .child(
                            div()
                                .flex()
                                .gap(px(10.))
                                .child(mode_card(
                                    "welcome-simple",
                                    "Simple",
                                    "One-click setup with recommended fixes, plus a short list of friendly settings.",
                                    Icon::Wand,
                                    Mode::Simple,
                                    true,
                                ))
                                .child(mode_card(
                                    "welcome-advanced",
                                    "Advanced",
                                    "Every setting, presets, exe patches and the mod manager.",
                                    Icon::Sliders,
                                    Mode::Advanced,
                                    false,
                                )),
                        ),
                )
                .child(ui::body("You can switch any time from the title bar.").text_color(theme::text_dim())),
            |_| {},
        ))
    }
}

impl Render for Shell {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let (page_kind, game_id, nav) = {
            let state = self.ws.read(cx);
            let def = state.game().def;
            let mode = state.mode();
            let nav = def
                .nav_items(mode)
                .find(|n| n.kind == state.page)
                .copied()
                .or_else(|| {
                    // Tool pages reached from elsewhere (e.g. App Settings).
                    matches!(state.page, PageKind::Capture).then_some(NavItem {
                        kind: state.page,
                        title: "Comparison Capture",
                        icon: Icon::Camera,
                        categories: &[],
                    })
                })
                .or_else(|| def.nav_items(mode).next().copied());
            (state.page, def.id, nav)
        };
        let page = match nav {
            Some(nav) => pages::render(&nav, &self.ws, window, cx),
            None => div().into_any_element(),
        };
        let scroll_id = SharedString::from(format!("page-{game_id}-{page_kind:?}"));
        // Two-pane pages scroll their own panes.
        let content = if pages::fills_height(page_kind) {
            div().flex_1().min_h_0().flex().flex_col().child(page)
        } else {
            div().flex_1().min_h_0().child(
                div().id(ElementId::Name(scroll_id)).size_full().overflow_y_scroll().child(
                    div()
                        .w_full()
                        .flex()
                        .justify_center()
                        .child(div().w_full().max_w(px(1120.)).px(px(28.)).pt(px(22.)).pb(px(40.)).child(page)),
                ),
            )
        };

        if window.focused(cx).is_none() {
            window.focus(&self.focus);
        }
        let pending_bar = self.pending_bar(cx);
        let toast_bottom = 40. + if pending_bar.is_some() { 52. } else { 0. };
        let drop_ws = self.ws.clone();
        div()
            .size_full()
            .relative()
            .flex()
            .flex_col()
            .track_focus(&self.focus)
            .on_key_down(cx.listener(Self::on_key))
            .bg(theme::bg())
            .font_family(theme::FONT_BODY)
            .text_color(theme::text())
            .text_size(px(14.))
            // Drop .sdkmod/.zip/.blcm files anywhere to install them as mods.
            .drag_over::<ExternalPaths>(|s, _, _, _| s.bg(theme::with_alpha(theme::accent(), 0.05)))
            .on_drop(move |paths: &ExternalPaths, _, cx| {
                let files: Vec<_> = paths.paths().to_vec();
                drop_ws.update(cx, |ws, cx| {
                    let mods_page = ws.game().def.nav_items(ws.mode()).any(|n| n.kind == PageKind::Mods);
                    ws.install_mod_files(files, cx);
                    if mods_page {
                        ws.navigate(PageKind::Mods, cx);
                    }
                });
            })
            .child(self.title_bar(window, cx))
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .flex()
                    .child(self.rail(cx))
                    .child(div().flex_1().min_w_0().h_full().flex().flex_col().child(content).children(pending_bar)),
            )
            .child(self.status_bar(cx))
            .children(crate::pages::compare::lightbox(&self.ws, window, cx))
            .children(self.review(cx))
            .children(self.welcome(cx))
            .child(self.toasts(toast_bottom, cx))
            .children(gpui_component::Root::render_dialog_layer(window, cx))
    }
}

/// Centered dialog over a dimmed backdrop; clicking the backdrop runs `on_backdrop`.
fn modal(id: &'static str, width: f32, content: impl IntoElement, on_backdrop: impl Fn(&mut gpui::App) + 'static) -> AnyElement {
    div()
        .id(id)
        .absolute()
        .inset_0()
        .flex()
        .items_center()
        .justify_center()
        .bg(gpui::hsla(0., 0., 0., 0.6))
        .occlude()
        .on_click(move |_, _, cx| on_backdrop(cx))
        .child(
            div()
                .id(SharedString::from(format!("{id}-body")))
                .w(px(width))
                .max_w_full()
                .bg(theme::panel())
                .border_1()
                .border_color(theme::line())
                .rounded(px(theme::RADIUS_LG))
                .shadow(theme::shadow())
                .overflow_hidden()
                .on_click(|_, _, cx| cx.stop_propagation())
                .child(content),
        )
        .into_any_element()
}

fn status_color(g: &crate::workspace::GameState) -> gpui::Rgba {
    if g.install.is_some() {
        theme::success()
    } else if g.config_found() {
        theme::warning()
    } else {
        theme::text_dim()
    }
}

fn status_text(g: &crate::workspace::GameState) -> &'static str {
    match (&g.install, g.config_found()) {
        (Some(i), _) => match i.store {
            crate::core::detect::Store::Steam => "Installed · Steam",
            crate::core::detect::Store::Epic => "Installed · Epic Games",
            crate::core::detect::Store::Manual => "Installed · custom folder",
        },
        (None, true) => "Settings found, game not installed",
        (None, false) => "Not found",
    }
}

/// The game's own icon (from its exe or Steam), or its short name.
pub fn game_icon(path: Option<std::path::PathBuf>, short: &'static str, size: f32) -> AnyElement {
    match path {
        Some(p) => img(p).size(px(size)).flex_none().object_fit(ObjectFit::Contain).into_any_element(),
        None => div()
            .size(px(size))
            .flex_none()
            .flex()
            .items_center()
            .justify_center()
            .rounded(px(theme::RADIUS))
            .bg(theme::panel_hi())
            .font_family(theme::FONT_TITLE)
            .font_weight(FontWeight::BOLD)
            .text_size(px(size * 0.42))
            .text_color(theme::text_muted())
            .child(short)
            .into_any_element(),
    }
}

fn chrome_button(id: &'static str, glyph: Icon, area: WindowControlArea, close: bool) -> impl IntoElement {
    div()
        .id(id)
        .w(px(46.))
        .h_full()
        .flex()
        .items_center()
        .justify_center()
        .window_control_area(area)
        .hover(move |s| if close { s.bg(theme::danger()) } else { s.bg(theme::panel_hi()) })
        .child(ui::icon(glyph).size(px(14.)).text_color(theme::text_muted()))
}

/// The app logo: a vault door on a hazard-yellow tile.
fn app_mark(size: f32) -> impl IntoElement {
    img(if size < 40. { theme::LOGO_SMALL } else { theme::LOGO }).size(px(size)).flex_none()
}
