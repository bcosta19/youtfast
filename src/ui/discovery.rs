//! Search filters and typed result lists.

use egui::{ComboBox, Ui};

use super::{library, results, widgets};
use crate::app::State;
use crate::innertube::{SearchDate, SearchKind, SearchLength, SearchOrder};
use crate::model::{Action, Listing};

pub fn filters(ui: &mut Ui, state: &State, actions: &mut Vec<Action>) {
    let mut options = state.search_options;
    ui.horizontal_wrapped(|ui| {
        for kind in SearchKind::ALL {
            if widgets::chip(ui, &state.palette, kind.label(), options.kind == *kind).clicked() {
                options.kind = *kind;
            }
        }
    });
    if options.kind == SearchKind::Videos {
        ui.horizontal_wrapped(|ui| {
            choice(
                ui,
                "search order",
                &mut options.order,
                SearchOrder::ALL,
                SearchOrder::label,
            );
            choice(
                ui,
                "search date",
                &mut options.date,
                SearchDate::ALL,
                SearchDate::label,
            );
            choice(
                ui,
                "search duration",
                &mut options.length,
                SearchLength::ALL,
                SearchLength::label,
            );
            ui.checkbox(&mut options.live, "Ao vivo");
        });
    }
    if options != state.search_options {
        actions.push(Action::SetSearchOptions(options));
    }
    ui.add_space(12.0);
}

fn choice<T: Copy + PartialEq>(
    ui: &mut Ui,
    id: &str,
    current: &mut T,
    values: &[T],
    label: impl Fn(T) -> &'static str,
) {
    ComboBox::from_id_salt(id)
        .selected_text(label(*current))
        .show_ui(ui, |ui| {
            for &value in values {
                ui.selectable_value(current, value, label(value));
            }
        });
}

pub fn other_results(ui: &mut Ui, state: &State, actions: &mut Vec<Action>) {
    match state.search_options.kind {
        SearchKind::Videos => {}
        SearchKind::Channels => match &state.results_channels {
            Listing::Idle => results::nothing(ui, state, "Pesquise um canal"),
            Listing::Loading => results::loading(ui, state, "canais"),
            Listing::Failed(error) => results::failed(ui, state, error, actions),
            Listing::Ready(channels) if channels.is_empty() => {
                results::nothing(ui, state, "Nenhum canal encontrado")
            }
            Listing::Ready(channels) => {
                egui::ScrollArea::vertical()
                    .auto_shrink(false)
                    .show(ui, |ui| {
                        library::channel_rows_content(ui, state, channels, false, actions);
                        widgets::more_row(
                            ui,
                            &state.palette,
                            &state.results_more_state,
                            state.results_more.is_some(),
                            actions,
                        );
                    });
            }
        },
        SearchKind::Playlists => match &state.results_playlists {
            Listing::Idle => results::nothing(ui, state, "Pesquise uma playlist"),
            Listing::Loading => results::loading(ui, state, "playlists"),
            Listing::Failed(error) => results::failed(ui, state, error, actions),
            Listing::Ready(playlists) if playlists.is_empty() => {
                results::nothing(ui, state, "Nenhuma playlist encontrada")
            }
            Listing::Ready(playlists) => {
                egui::ScrollArea::vertical()
                    .auto_shrink(false)
                    .show(ui, |ui| {
                        results::virtual_rows(
                            ui,
                            106.0,
                            playlists.len(),
                            state.cursor_moved.then_some(state.cursor),
                            |ui, index| {
                                let playlist = &playlists[index];
                                let response = widgets::playlist_row(
                                    ui,
                                    &state.palette,
                                    playlist,
                                    index == state.cursor,
                                    true,
                                );
                                if response.clicked() {
                                    actions.push(Action::OpenPlaylist {
                                        id: playlist.id.clone(),
                                        name: playlist.name.clone(),
                                    });
                                }
                            },
                        );
                        widgets::more_row(
                            ui,
                            &state.palette,
                            &state.results_more_state,
                            state.results_more.is_some(),
                            actions,
                        );
                    });
            }
        },
    }
}
