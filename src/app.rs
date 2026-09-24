//! The main window: custom title bar, game switcher + navigation sidebar,
//! the active page, the staged-changes bar and toasts.

use gpui::{
    AnyElement, Context, ElementId, Entity, FontWeight, IntoElement, ParentElement, Render,
    SharedString, Styled, Subscription, Window, WindowControlArea, div, prelude::*, px,
};

use crate::games::{Mode, NavItem};
use crate::pages;
use crate::theme::{self, Icon};
use crate::ui::{self, Variant};
use crate::workspace::{ToastKind, Workspace};

pub struct Shell {
    ws: Entity<Workspace>,
    _observe: Subscription,
}

impl Shell {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let ws = cx.new(Workspace::new);
        let observe = cx.observe(&ws, |_, _, cx| cx.notify());
        Self { ws, _observe: observe }
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
                    .child(
                        div()
                            .font_family(theme::FONT_LABEL)
                            .text_size(px(12.))
                            .text_color(theme::text_dim())
                            .child(format!("// {}", state.game().def.name.to_uppercase())),
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
                    .text_size(px(12.))
                    .cursor_pointer()
                    .when(!active, |d| d.hover(|s| s.text_color(theme::text())))
                    .child(ui::icon(icon).text_size(px(11.)))
                    .child(text.to_uppercase())
                    .on_click(move |_, _, cx| ws.update(cx, |ws, cx| ws.set_mode(m, cx))),
            );
        }
        row
    }

    fn sidebar(&self, cx: &Context<Self>) -> impl IntoElement {
        let state = self.ws.read(cx);
        let mut games_row = div().flex().gap(px(6.)).px(px(12.)).pt(px(14.));
        for (i, g) in state.games.iter().enumerate() {
            let active = i == state.active;
            let found = g.install.is_some() || g.config_found();
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
                    .child(div().size(px(6.)).bg(if found {
                        theme::success()
                    } else {
                        theme::text_dim()
                    }))
                    .on_click(move |_, _, cx| ws.update(cx, |ws, cx| ws.select_game(i, cx))),
            );
        }

        let game = state.game();
        let mut nav = div().flex().flex_col().gap(px(2.)).px(px(10.)).pb(px(16.));
        let groups = game.def.nav_for(state.mode()).iter();
        for group in groups {
            nav = nav.child(ui::label(group.title).text_color(theme::text_dim()).px(px(8.)).pt(px(16.)).pb(px(4.)));
            for item in group.items {
                nav = nav.child(self.nav_button(item, cx));
            }
        }

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
    }

    fn nav_button(&self, item: &'static NavItem, cx: &Context<Self>) -> impl IntoElement {
        let state = self.ws.read(cx);
        let game = state.game();
        let active = state.page == item.kind;
        let staged = game
            .pending
            .keys()
            .filter(|id| {
                game.def
                    .tweak(id)
                    .is_some_and(|t| item.categories.contains(&t.category))
            })
            .count();
        let ws = self.ws.clone();
        let kind = item.kind;
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
                    .text_size(px(14.))
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
                        .text_size(px(11.))
                        .child(staged.to_string()),
                )
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

    fn toasts(&self, cx: &Context<Self>) -> impl IntoElement {
        let state = self.ws.read(cx);
        let mut stack = div()
            .absolute()
            .bottom(px(84.))
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
                .or_else(|| def.nav_items(mode).next())
                .copied();
            (state.page, def.id, nav)
        };
        let page = match nav {
            Some(nav) => pages::render(&nav, &self.ws, window, cx),
            None => div().into_any_element(),
        };
        let scroll_id = SharedString::from(format!("page-{game_id}-{page_kind:?}"));

        div()
            .size_full()
            .relative()
            .flex()
            .flex_col()
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
                            .children(self.pending_bar(cx)),
                    ),
            )
            .children(crate::pages::compare::lightbox(&self.ws, cx))
            .child(self.toasts(cx))
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
