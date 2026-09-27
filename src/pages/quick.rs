//! Simple mode's settings page: the settings that matter most, in the shared
//! list + detail layout, saved as soon as they change.

use gpui::{AnyElement, App, Entity, FontWeight, IntoElement, ParentElement, SharedString, Styled, Window, div, prelude::*, px};

use super::tweaks::{Group, Notice, search_filter, view};
use crate::controls;
use crate::theme;
use crate::ui;
use crate::workspace::Workspace;

pub fn render(ws: &Entity<Workspace>, window: &mut Window, cx: &mut App) -> AnyElement {
    let state = ws.read(cx);
    let game = state.game();
    let def = game.def;
    let query = state.search.trim().to_lowercase();

    let mut groups = Vec::new();
    for section in def.quick {
        let tweaks: Vec<_> = section
            .tweaks
            .iter()
            .filter_map(|id| def.tweak(id))
            .filter(|t| search_filter(t, section.title, &query))
            .collect();
        if tweaks.is_empty() {
            continue;
        }
        // One-click quality levels above the graphics settings, as selection
        // cards (RadioButtons): pick one, the current one is checked.
        let extra = (!section.quality_presets.is_empty() && query.is_empty()).then(|| {
            let mut row = div().flex().flex_wrap().gap(px(8.));
            for id in section.quality_presets {
                let Some(preset) = def.presets.iter().find(|p| p.id == *id) else { continue };
                let current = !preset.values.is_empty()
                    && preset.values.iter().all(|(id, v)| def.tweak(id).is_none_or(|t| game.effective(t) == v.to_value()));
                let ws = ws.clone();
                row = row.child(
                    ui::focusable(div().id(SharedString::from(format!("q-preset-{id}"))))
                        .flex_1()
                        .flex_basis(px(0.))
                        .min_w(px(160.))
                        .flex()
                        .items_start()
                        .gap(px(12.))
                        .rounded(px(theme::RADIUS))
                        // A 2px accent border marks the choice; padding
                        // absorbs the extra pixel so nothing shifts.
                        .map(|d| if current { d.border_2().border_color(theme::accent()).p(px(11.)) } else { d.border_1().border_color(theme::card_stroke()).p(px(12.)) })
                        .bg(theme::panel())
                        .cursor_pointer()
                        .hover(|s| s.bg(theme::panel_hi()))
                        .active(|s| s.bg(theme::panel_pressed()))
                        .tooltip(ui::tip(preset.description))
                        .child(controls::radio(current))
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .flex()
                                .flex_col()
                                .gap(px(2.))
                                .child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .gap(px(6.))
                                        .child(div().size(px(8.)).flex_none().rounded_full().bg(preset.rarity.color()))
                                        .child(div().flex_1().min_w_0().line_clamp(1).text_ellipsis().text_size(px(14.)).line_height(px(20.)).font_weight(FontWeight::SEMIBOLD).text_color(theme::text()).child(preset.name)),
                                )
                                .child(div().min_w_0().text_size(px(12.)).line_height(px(16.)).text_color(theme::text_muted()).line_clamp(2).text_ellipsis().child(preset.description)),
                        )
                        .on_mouse_down(gpui::MouseButton::Left, |_, _, _| crate::sound::play(crate::sound::Sound::Click))
                        .on_click(move |_, _, cx| ws.update(cx, |ws, cx| ws.preset_now(preset, cx))),
                );
            }
            row.into_any_element()
        });
        groups.push(Group { title: section.title.into(), blurb: section.blurb.into(), tweaks, extra });
    }

    let notice = (!game.config_found()).then(|| Notice {
        title: "Settings files not created yet".into(),
        message: format!("{} hasn't created its settings files yet. Launch it once, then come back, or set the folder in App settings.", def.name),
    });
    view(
        ws,
        "Quick settings",
        "Saved as soon as you change them. Want every setting? Switch to Advanced at the top of the navigation pane.",
        groups,
        true,
        None,
        None,
        notice,
        window,
        cx,
    )
}
