//! Simple mode's settings page: the settings that matter most, in the shared
//! list + detail layout, saved as soon as they change.

use gpui::{AnyElement, App, Entity, FontWeight, IntoElement, ParentElement, SharedString, Styled, Window, div, prelude::*, px};

use super::tweaks::{Group, search_filter, view};
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
        // One-click quality levels above the graphics settings.
        let extra = (!section.quality_presets.is_empty() && query.is_empty()).then(|| {
            let mut row = div().flex().gap(px(8.));
            for id in section.quality_presets {
                let Some(preset) = def.presets.iter().find(|p| p.id == *id) else { continue };
                let current = !preset.values.is_empty()
                    && preset.values.iter().all(|(id, v)| def.tweak(id).is_none_or(|t| game.effective(t) == v.to_value()));
                let ws = ws.clone();
                row = row.child(
                    div()
                        .id(SharedString::from(format!("q-preset-{id}")))
                        .flex_1()
                        .flex_basis(px(0.))
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .gap(px(2.))
                        .px(px(12.))
                        .py(px(10.))
                        .rounded(px(theme::RADIUS_LG))
                        .border_1()
                        .border_color(if current { theme::accent() } else { theme::line() })
                        .bg(if current { theme::selected() } else { theme::panel() })
                        .cursor_pointer()
                        .hover(|s| s.bg(theme::panel_hi()))
                        .tooltip(ui::tip(preset.description))
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap(px(6.))
                                .child(div().size(px(8.)).rounded_full().bg(preset.rarity.color()))
                                .child(div().text_size(px(14.)).font_weight(FontWeight::SEMIBOLD).child(preset.name))
                                .when(current, |d| d.child(ui::badge("Current", theme::success()))),
                        )
                        .child(div().text_size(px(12.)).text_color(theme::text_muted()).truncate().child(preset.description))
                        .on_mouse_down(gpui::MouseButton::Left, |_, _, _| crate::sound::play(crate::sound::Sound::Click))
                        .on_click(move |_, _, cx| ws.update(cx, |ws, cx| ws.preset_now(preset, cx))),
                );
            }
            row.into_any_element()
        });
        groups.push(Group { title: section.title.into(), blurb: section.blurb.into(), tweaks, extra });
    }

    let notice = (!game.config_found()).then(|| format!("{} hasn't created its settings files yet. Launch it once, then come back.", def.name));
    view(
        ws,
        "Quick Settings",
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
