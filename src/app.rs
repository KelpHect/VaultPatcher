//! The main window: custom title bar, game switcher + navigation sidebar,
//! the active page, the staged-changes bar and toasts.

use gpui::{
    AnyElement, Context, ElementId, Entity, FontWeight, IntoElement, ParentElement, Render,
    SharedString, Styled, Subscription, Window, WindowControlArea, div, prelude::*, px,
};

use crate::games::{Mode, NavItem, PageKind};
use crate::pages;
use crate::theme::{self, Icon};
use crate::ui::{self, Variant};
use crate::workspace::{ToastKind, Workspace};

pub struct Shell {
    ws: Entity<Workspace>,
    focus: gpui::FocusHandle,
    _observe: Subscription,
}

impl Shell {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let ws = cx.new(Workspace::new);
        let observe = cx.observe(&ws, |_, _, cx| cx.notify());
        Self { ws, focus: cx.focus_handle(), _observe: observe }
    }

    /// App-wide keys: Esc closes the viewer (or discards nothing — it never
    /// deletes work), ←/→ flip the viewer's right side, Ctrl+S applies.
    fn on_key(&mut self, ev: &gpui::KeyDownEvent, _: &mut Window, cx: &mut Context<Self>) {
        let key = ev.keystroke.key.as_str();
        let ctrl = ev.keystroke.modifiers.control;
        self.ws.update(cx, |ws, cx| {
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

    fn title_bar(&self, window: &Window, cx: &Context<Self>) -> impl IntoElement {
        let state = self.ws.read(cx);
        let maximized = window.is_maximized();
        div()
            .h(px(44.))
            .flex_none()
            .flex()
            .items_center()
            .bg(theme::bg_deep())
            .border_b_2()
            .border_color(theme::ink())
            .child(
                div()
                    .id("drag")
                    .flex_1()
                    .h_full()
                    .flex()
                    .items_center()
                    .gap(px(12.))
                    .pl(px(16.))
                    .window_control_area(WindowControlArea::Drag)
                    .child(logo())
                    .child(
                        div()
                            .font_family(theme::FONT_DISPLAY)
                            .text_size(px(24.))
                            .text_color(theme::accent())
                            .child("VAULT PATCHER"),
                    )
                    .when_some(state.busy.clone(), |d, b| {
                        d.child(ui::badge(b, theme::accent()))
                    }),
            )
            .child(self.mode_toggle(cx))
            .child(chrome_button("min", Icon::Minimize, WindowControlArea::Min, false))
            .child(chrome_button(
                "max",
                if maximized { Icon::Restore } else { Icon::Maximize },
                WindowControlArea::Max,
                false,
            ))
            .child(chrome_button("close", Icon::Close, WindowControlArea::Close, true))
    }

    /// Simple / Advanced switch in the title bar.
    fn mode_toggle(&self, cx: &Context<Self>) -> impl IntoElement {
        let mode = self.ws.read(cx).mode();
        let mut row = div()
            .flex()
            .items_center()
            .mr(px(14.))
            .border_1()
            .border_color(theme::line())
            .bg(theme::panel_lo());
        for (m, text, icon) in [
            (Mode::Simple, "Simple", Icon::Sparkle),
            (Mode::Advanced, "Advanced", Icon::Sliders),
        ] {
            let active = m == mode;
            let ws = self.ws.clone();
            row = row.child(
                div()
                    .id(SharedString::from(format!("mode-{text}")))
                    .flex()
                    .items_center()
                    .gap(px(6.))
                    .h(px(26.))
                    .px(px(12.))
                    .bg(if active { theme::panel_hi() } else { gpui::transparent_black().into() })
                    .text_color(if active { theme::text() } else { theme::text_dim() })
                    .font_family(theme::FONT_LABEL)
                    .font_weight(FontWeight::BOLD)
                    .text_size(px(13.))
                    .cursor_pointer()
                    .when(!active, |d| d.hover(|s| s.text_color(theme::text())))
                    .child(ui::icon(icon).text_size(px(11.)))
                    .child(text.to_uppercase())
                    .on_click(move |_, window, cx| {
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
                    }),
            );
        }
        row
    }

    fn sidebar(&self, cx: &Context<Self>) -> impl IntoElement {
        let state = self.ws.read(cx);
        let mut games_row = div().flex().gap(px(6.)).px(px(12.)).pt(px(14.));
        for (i, g) in state.games.iter().enumerate() {
            let active = i == state.active;
            // Installed (green), settings only (amber), or not found (grey).
            let dot = if g.install.is_some() {
                theme::success()
            } else if g.config_found() {
                crate::theme::Rarity::Legendary.color()
            } else {
                theme::text_dim()
            };
            let ws = self.ws.clone();
            games_row = games_row.child(
                div()
                    .id(SharedString::from(format!("game-{}", g.def.id)))
                    .flex_1()
                    .flex()
                    .flex_col()
                    .items_center()
                    .py(px(6.))
                    .border_2()
                    .border_color(theme::ink())
                    .bg(if active { theme::accent() } else { theme::panel() })
                    .when(active, |d| d.shadow(theme::comic_shadow(3.)))
                    .when(!active, |d| d.hover(|s| s.bg(theme::panel_hi())))
                    .cursor_pointer()
                    .child(
                        div()
                            .font_family(theme::FONT_DISPLAY)
                            .text_size(px(22.))
                            .text_color(if active { theme::accent_ink() } else { theme::text() })
                            .child(g.def.short),
                    )
                    .child(div().size(px(8.)).bg(dot))
                    .on_click(move |_, _, cx| ws.update(cx, |ws, cx| ws.select_game(i, cx))),
            );
        }

        let game = state.game();
        let mut nav = div().flex().flex_col().gap(px(2.)).px(px(10.)).pb(px(16.));
        let mut previous_was_tabs = false;
        for group in game.def.nav_for(state.mode()) {
            if let Some(icon) = group.tabs {
                // One entry for the whole group; its pages are tabs.
                let active = group.items.iter().any(|i| i.kind == state.page);
                let categories: Vec<&str> = group.items.iter().flat_map(|i| i.categories.iter().copied()).collect();
                nav = nav.child(self.nav_entry(group.title, icon, group.items[0].kind, active, &categories, cx));
                previous_was_tabs = true;
                continue;
            }
            nav = nav.child(
                ui::label(group.title)
                    .text_color(theme::text_dim())
                    .px(px(8.))
                    .pt(px(if previous_was_tabs { 12. } else { 16. }))
                    .pb(px(4.)),
            );
            previous_was_tabs = false;
            for item in group.items {
                nav = nav.child(self.nav_button(item, cx));
            }
        }

        // PLAY is always one click away, in both modes.
        let play_ws = self.ws.clone();
        let can_play = game.install.is_some();
        let play = div().p(px(12.)).border_t_1().border_color(theme::line()).child(
            ui::button("sidebar-play", format!("Play {}", game.def.short), Some(Icon::Play), ui::Variant::Primary)
                .w_full()
                .justify_center()
                .h(px(44.))
                .text_size(px(17.))
                .when(!can_play, |b| b.opacity(0.4))
                .on_click(move |_, _, cx| {
                    if can_play {
                        play_ws.update(cx, |ws, cx| ws.launch(cx))
                    }
                }),
        );

        div()
            .w(px(250.))
            .flex_none()
            .h_full()
            .flex()
            .flex_col()
            .bg(theme::bg_deep())
            .border_r_2()
            .border_color(theme::ink())
            .child(games_row)
            .child(
                div()
                    .id("nav-scroll")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .child(nav),
            )
            .child(play)
    }

    fn nav_button(&self, item: &'static NavItem, cx: &Context<Self>) -> impl IntoElement {
        let active = self.ws.read(cx).page == item.kind;
        self.nav_entry(item.title, item.icon, item.kind, active, item.categories, cx)
    }

    fn nav_entry(
        &self,
        title: &'static str,
        icon: Icon,
        kind: crate::games::PageKind,
        active: bool,
        categories: &[&str],
        cx: &Context<Self>,
    ) -> impl IntoElement {
        let state = self.ws.read(cx);
        let game = state.game();
        let staged = game
            .pending
            .keys()
            .filter(|id| game.def.tweak(id).is_some_and(|t| categories.contains(&t.category)))
            .count();
        let ws = self.ws.clone();
        let item = NavItem { kind, title, icon, categories: &[] };
        div()
            .id(SharedString::from(format!("nav-{}", item.title)))
            .flex()
            .items_center()
            .gap(px(10.))
            .h(px(34.))
            .px(px(10.))
            .border_l_4()
            .border_color(if active { theme::accent() } else { gpui::transparent_black().into() })
            .bg(if active { theme::panel_hi() } else { gpui::transparent_black().into() })
            .hover(|s| s.bg(theme::panel()))
            .cursor_pointer()
            .child(
                ui::icon(item.icon)
                    .text_size(px(14.))
                    .text_color(if active { theme::accent() } else { theme::text_muted() }),
            )
            .child(
                div()
                    .flex_1()
                    .font_family(theme::FONT_LABEL)
                    .font_weight(if active { FontWeight::BOLD } else { FontWeight::MEDIUM })
                    .text_size(px(16.5))
                    .text_color(if active { theme::text() } else { theme::text_muted() })
                    .child(item.title),
            )
            .when(staged > 0, |d| {
                d.child(
                    div()
                        .px(px(6.))
                        .bg(theme::accent())
                        .text_color(theme::accent_ink())
                        .font_family(theme::FONT_LABEL)
                        .font_weight(FontWeight::BOLD)
                        .text_size(px(12.))
                        .child(staged.to_string()),
                )
            })
            .on_hover(|hovered, _, _| {
                if *hovered {
                    crate::sound::play(crate::sound::Sound::Hover)
                }
            })
            .on_click(move |_, _, cx| ws.update(cx, |ws, cx| ws.navigate(kind, cx)))
    }

    fn pending_bar(&self, cx: &Context<Self>) -> Option<AnyElement> {
        let state = self.ws.read(cx);
        let game = state.game();
        // Simple mode saves automatically, so there's nothing to confirm.
        if game.pending.is_empty() || state.mode() == Mode::Simple {
            return None;
        }
        let names: Vec<&str> = game
            .pending
            .keys()
            .filter_map(|id| game.def.tweak(id).map(|t| t.label))
            .take(4)
            .collect();
        let more = game.pending.len().saturating_sub(names.len());
        let discard_ws = self.ws.clone();
        let apply_ws = self.ws.clone();
        Some(
            div()
                .flex_none()
                .h(px(64.))
                .flex()
                .items_center()
                .bg(theme::panel_hi())
                .border_t_2()
                .border_color(theme::ink())
                .child(
                    div()
                        .w(px(26.))
                        .h_full()
                        .bg(theme::accent())
                        .child(ui::hazard_stripes(theme::ink(), 16.).size_full()),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .px(px(18.))
                        .flex()
                        .flex_col()
                        .child(
                            div()
                                .font_family(theme::FONT_DISPLAY)
                                .text_size(px(24.))
                                .text_color(theme::accent())
                                .child(format!("{} change(s) staged", game.pending.len())),
                        )
                        .child(
                            div()
                                .text_size(px(12.))
                                .text_color(theme::text_muted())
                                .truncate()
                                .child(format!(
                                    "{}{}",
                                    names.join(" · "),
                                    if more > 0 { format!(" · +{more} more") } else { String::new() }
                                )),
                        ),
                )
                .child(
                    div()
                        .flex()
                        .gap(px(12.))
                        .pr(px(20.))
                        .child(
                            ui::button("discard", "Discard", Some(Icon::Close), Variant::Secondary)
                                .on_click(move |_, _, cx| discard_ws.update(cx, |ws, cx| ws.discard_pending(cx))),
                        )
                        .child(
                            ui::button("apply", "Apply changes", Some(Icon::Check), Variant::Primary)
                                .on_click(move |_, _, cx| apply_ws.update(cx, |ws, cx| ws.apply_pending(cx))),
                        ),
                )
                .into_any_element(),
        )
    }

    /// Bottom status bar: detection state, setup progress, sound controls.
    fn status_bar(&self, cx: &Context<Self>) -> impl IntoElement {
        use crate::mods::SdkStatus;
        let state = self.ws.read(cx);
        let game = state.game();
        let found = game.install.is_some();
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

        let item = |text: String| {
            div()
                .flex_none()
                .font_family(theme::FONT_LABEL)
                .font_weight(FontWeight::MEDIUM)
                .text_size(px(13.))
                .text_color(theme::text_dim())
                .child(text)
        };
        let sep = || div().w(px(1.)).h(px(14.)).bg(theme::line());
        let toggle = |id: &'static str, glyph: Icon, on: bool| {
            div()
                .id(id)
                .flex()
                .items_center()
                .justify_center()
                .size(px(26.))
                .text_color(if on { theme::accent() } else { theme::text_dim() })
                .cursor_pointer()
                .hover(|s| s.bg(theme::panel_hi()).text_color(theme::text()))
                .child(ui::icon(glyph).text_size(px(12.)))
        };

        div()
            .h(px(32.))
            .flex_none()
            .flex()
            .items_center()
            .gap(px(12.))
            .px(px(14.))
            .bg(theme::bg_deep())
            .border_t_1()
            .border_color(theme::line())
            .child(div().size(px(8.)).flex_none().bg(if found { theme::success() } else { theme::danger() }))
            .child(
                div()
                    .flex_none()
                    .font_family(theme::FONT_LABEL)
                    .font_weight(FontWeight::BOLD)
                    .text_size(px(13.))
                    .text_color(theme::text())
                    .child(game.def.name.to_uppercase()),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .text_size(px(12.5))
                    .text_color(theme::text_dim())
                    .child(where_),
            )
            .child(item(sdk))
            .child(sep())
            .child(item(format!("{active} of {} upgrades", game.def.setup.len())))
            .when_some(state.busy.clone(), |d, b| d.child(sep()).child(item(b).text_color(theme::accent())))
            .child(sep())
            .child(
                toggle("sb-music", Icon::Music, music && !muted && sounds)
                    .when(!sounds, |d| d.opacity(0.4))
                    .on_click(move |_, _, cx| music_ws.update(cx, |ws, cx| ws.set_music(!music, cx))),
            )
            .child(
                toggle("sb-mute", if muted { Icon::Mute } else { Icon::Volume }, !muted && sounds)
                    .when(!sounds, |d| d.opacity(0.4))
                    .on_click(move |_, _, cx| mute_ws.update(cx, |ws, cx| ws.set_muted(!muted, cx))),
            )
            .child(item(if sounds { "Game sounds".into() } else { "No game sounds found".into() }))
            .child(sep())
            .child(item(format!("v{}", env!("CARGO_PKG_VERSION"))))
    }

    fn toasts(&self, bottom: f32, cx: &Context<Self>) -> impl IntoElement {
        let state = self.ws.read(cx);
        let mut stack = div()
            .absolute()
            .bottom(px(bottom))
            .right(px(24.))
            .w(px(380.))
            .flex()
            .flex_col()
            .gap(px(10.));
        for toast in &state.toasts {
            let color = match toast.kind {
                ToastKind::Info => theme::echo(),
                ToastKind::Success => theme::success(),
                ToastKind::Error => theme::danger(),
            };
            let icon = match toast.kind {
                ToastKind::Info => Icon::Info,
                ToastKind::Success => Icon::Check,
                ToastKind::Error => Icon::Warning,
            };
            let ws = self.ws.clone();
            let id = toast.id;
            stack = stack.child(
                div()
                    .id(ElementId::Integer(id))
                    .flex()
                    .bg(theme::panel_hi())
                    .border_2()
                    .border_color(theme::ink())
                    .shadow(theme::comic_shadow(4.))
                    .cursor_pointer()
                    .child(div().w(px(6.)).bg(color))
                    .child(
                        div()
                            .flex_1()
                            .flex()
                            .items_start()
                            .gap(px(10.))
                            .p(px(12.))
                            .child(ui::icon(icon).text_color(color).text_size(px(15.)).pt(px(2.)))
                            .child(
                                div()
                                    .flex_1()
                                    .text_size(px(13.))
                                    .text_color(theme::text())
                                    .child(toast.message.clone()),
                            ),
                    )
                    .on_click(move |_, _, cx| ws.update(cx, |ws, cx| ws.dismiss_toast(id, cx))),
            );
        }
        stack
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
                        title: "",
                        icon: Icon::Home,
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

        if window.focused(cx).is_none() {
            window.focus(&self.focus);
        }
        let pending_bar = self.pending_bar(cx);
        let toast_bottom = 44. + if pending_bar.is_some() { 64. } else { 0. };
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
            .child(self.title_bar(window, cx))
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .flex()
                    .child(self.sidebar(cx))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .h_full()
                            .flex()
                            .flex_col()
                            .child(
                                div()
                                    .id(ElementId::Name(scroll_id))
                                    .flex_1()
                                    .min_h_0()
                                    .overflow_y_scroll()
                                    .child(
                                        // A centered, readable column instead of edge-to-edge lines.
                                        div().w_full().flex().justify_center().child(
                                            div().w_full().max_w(px(1040.)).px(px(36.)).pt(px(30.)).pb(px(56.)).child(page),
                                        ),
                                    ),
                            )
                            .children(pending_bar),
                    ),
            )
            .child(self.status_bar(cx))
            .children(crate::pages::compare::lightbox(&self.ws, window, cx))
            .child(self.toasts(toast_bottom, cx))
    }
}

fn chrome_button(id: &'static str, glyph: Icon, area: WindowControlArea, close: bool) -> impl IntoElement {
    div()
        .id(id)
        .w(px(48.))
        .h_full()
        .flex()
        .items_center()
        .justify_center()
        .text_color(theme::text_muted())
        .window_control_area(area)
        .hover(move |s| {
            if close {
                s.bg(theme::danger()).text_color(theme::text())
            } else {
                s.bg(theme::panel_hi()).text_color(theme::text())
            }
        })
        .child(ui::icon(glyph).text_size(px(10.)))
}

/// The app mark: a vault-hunter style "VP" badge.
fn logo() -> impl IntoElement {
    div()
        .size(px(28.))
        .flex()
        .items_center()
        .justify_center()
        .bg(theme::accent())
        .border_2()
        .border_color(theme::ink())
        .shadow(theme::comic_shadow(2.))
        .child(
            div()
                .font_family(theme::FONT_DISPLAY)
                .text_size(px(18.))
                .text_color(theme::accent_ink())
                .child("VP"),
        )
}
