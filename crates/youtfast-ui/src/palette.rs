//! Command palette (`Ctrl+K` / `Cmd+K`): a fuzzy-filtered modal over views,
//! player and page actions, Settings groups, recent searches, and recent
//! watch history entries. All data is local and in-memory; opening or
//! filtering never makes network requests.

use egui::text::LayoutJob;
use egui::{CornerRadius, CursorIcon, Key, Rect, Sense, Stroke, Ui, pos2, vec2};

use super::{SidebarButton, sidebar_buttons, widgets};
use youtfast_app::{AccountState, Action, PlayerMenu, RECENT_SEARCHES_LIMIT, State};
use youtfast_appearance::icons::Icon;
use youtfast_appearance::theme::{self, RADIUS};
use youtfast_core::model::{SettingsGroup, View};
use youtfast_i18n::gettext as tr;
use youtfast_mpv::Status;

/// Maximum number of recent history entries included in the command palette.
pub const RECENT_HISTORY_LIMIT: usize = 25;

const ROW_HEIGHT: f32 = 38.0;

/// Widget ID for the command palette's search text field.
pub fn input_id() -> egui::Id {
    egui::Id::new("command-palette-input")
}

fn moved_id() -> egui::Id {
    egui::Id::new("command-palette-cursor-moved")
}

fn was_open_id() -> egui::Id {
    egui::Id::new("command-palette-was-open")
}

fn focus_pending_id() -> egui::Id {
    egui::Id::new("command-palette-focus-pending")
}

fn mark_cursor_moved(ctx: &egui::Context) {
    ctx.data_mut(|data| data.insert_temp(moved_id(), true));
}

fn take_cursor_moved(ctx: &egui::Context) -> bool {
    ctx.data_mut(|data| {
        let moved = data.get_temp::<bool>(moved_id()).unwrap_or(false);
        data.insert_temp(moved_id(), false);
        moved
    })
}

/// Opens the command palette, closing other overlays and focusing the search input.
pub fn open(ctx: &egui::Context, state: &mut State) {
    state.palette_open = true;
    state.palette_query.clear();
    state.palette_cursor = 0;
    state.keys_open = false;
    state.notifications_popup_open = false;
    state.player_menu = PlayerMenu::Closed;
    egui::Popup::close_all(ctx);
    ctx.data_mut(|data| {
        data.insert_temp(was_open_id(), true);
        data.insert_temp(focus_pending_id(), true);
    });
    mark_cursor_moved(ctx);
    ctx.memory_mut(|memory| memory.request_focus(input_id()));
}

/// Closes the command palette and clears its search state.
pub fn close(ctx: &egui::Context, state: &mut State) {
    state.palette_open = false;
    state.palette_query.clear();
    state.palette_cursor = 0;
    ctx.data_mut(|data| {
        data.insert_temp(was_open_id(), false);
        data.insert_temp(focus_pending_id(), false);
    });
    ctx.memory_mut(|memory| {
        if memory.has_focus(input_id()) {
            memory.surrender_focus(input_id());
        }
    });
}

/// Toggles the command palette open or closed.
pub fn toggle(ctx: &egui::Context, state: &mut State) {
    if state.palette_open {
        close(ctx, state);
    } else {
        open(ctx, state);
    }
}

/// One candidate entry in the command palette.
#[derive(Clone, Debug, PartialEq)]
pub struct PaletteItem {
    pub category: String,
    pub label: String,
    pub detail: Option<String>,
    pub keywords: Option<String>,
    pub shortcut: Option<String>,
    pub icon: Icon,
    pub actions: Vec<Action>,
    pub search_query: Option<String>,
}

/// Builds the full unfiltered list of command palette items in logical order:
/// views, page/player actions, Settings groups, recent searches, and recent history.
pub fn candidate_items(state: &State, status: Option<&Status>) -> Vec<PaletteItem> {
    let locale = state.settings.language;
    let mut items = Vec::with_capacity(48);

    // 1. Views (the sidebar's buttons)
    let view_category = tr(locale, "View").into_owned();
    for (index, button) in sidebar_buttons(state).enumerate() {
        let shortcut = (index < 9).then(|| (index + 1).to_string());
        let (icon, view, label, detail, keywords) = match button {
            SidebarButton::Home => (
                Icon::Home,
                View::Home,
                tr(locale, "Home").into_owned(),
                None,
                None,
            ),
            SidebarButton::Subscriptions => (
                Icon::Subscriptions,
                View::Subscriptions,
                tr(locale, "Subscriptions").into_owned(),
                None,
                None,
            ),
            SidebarButton::History => (
                Icon::History,
                View::History,
                tr(locale, "History").into_owned(),
                None,
                None,
            ),
            SidebarButton::WatchLater => (
                Icon::Clock,
                View::WatchLater,
                tr(locale, "Watch later").into_owned(),
                None,
                None,
            ),
            SidebarButton::Liked => (
                Icon::ThumbsUp,
                View::Liked,
                tr(locale, "Liked videos").into_owned(),
                None,
                None,
            ),
            SidebarButton::Playlists => (
                Icon::Playlists,
                View::Playlists,
                tr(locale, "Playlists").into_owned(),
                None,
                None,
            ),
            SidebarButton::Notifications => (
                Icon::Bell,
                View::Notifications,
                tr(locale, "Notifications").into_owned(),
                None,
                None,
            ),
            SidebarButton::Account => {
                let detail = state.active_account().map_or_else(
                    || match &state.account {
                        AccountState::SigningIn => Some(tr(locale, "Signing in").into_owned()),
                        _ => Some(tr(locale, "Sign in").into_owned()),
                    },
                    |account| Some(account.name.clone()),
                );
                (
                    Icon::User,
                    View::Account,
                    tr(locale, "Account").into_owned(),
                    detail,
                    Some(tr(locale, "Sign in").into_owned()),
                )
            }
            SidebarButton::Settings => (
                Icon::Settings,
                View::Settings,
                tr(locale, "Settings").into_owned(),
                None,
                None,
            ),
            SidebarButton::Unknown => continue,
        };
        items.push(PaletteItem {
            category: view_category.clone(),
            label,
            detail,
            keywords,
            shortcut,
            icon,
            actions: vec![Action::Navigate(view)],
            search_query: None,
        });
    }

    // 2. Page and player actions
    let action_category = tr(locale, "Action").into_owned();
    items.push(PaletteItem {
        category: action_category.clone(),
        label: tr(locale, "Refresh page").into_owned(),
        detail: None,
        keywords: None,
        shortcut: Some(tr(locale, "Ctrl/Cmd+R").into_owned()),
        icon: Icon::Refresh,
        actions: vec![Action::Refresh],
        search_query: None,
    });
    items.push(PaletteItem {
        category: action_category.clone(),
        label: tr(locale, "Toggle sidebar").into_owned(),
        detail: None,
        keywords: Some(tr(locale, "Sidebar").into_owned()),
        shortcut: Some("b".into()),
        icon: Icon::PanelLeft,
        actions: vec![Action::ToggleSidebar],
        search_query: None,
    });
    if state.can_go_back {
        items.push(PaletteItem {
            category: action_category.clone(),
            label: tr(locale, "Back").into_owned(),
            detail: None,
            keywords: None,
            shortcut: (!cfg!(target_os = "macos")).then(|| tr(locale, "Alt+←").into_owned()),
            icon: Icon::Back,
            actions: vec![Action::Back],
            search_query: None,
        });
    }
    if state.can_go_forward {
        items.push(PaletteItem {
            category: action_category.clone(),
            label: tr(locale, "Forward").into_owned(),
            detail: None,
            keywords: None,
            shortcut: (!cfg!(target_os = "macos")).then(|| tr(locale, "Alt+→").into_owned()),
            icon: Icon::Forward,
            actions: vec![Action::Forward],
            search_query: None,
        });
    }
    if state.watching.is_some() || state.video_tabs.videos().next().is_some() {
        items.push(PaletteItem {
            category: action_category,
            label: tr(locale, "New video tab").into_owned(),
            detail: None,
            keywords: Some(tr(locale, "New tab").into_owned()),
            shortcut: None,
            icon: Icon::Plus,
            actions: vec![Action::NewVideoTab],
            search_query: None,
        });
    }

    if state.watching.is_some() {
        let player_category = tr(locale, "Player").into_owned();
        let paused = status.is_none_or(|s| s.paused);
        items.push(PaletteItem {
            category: player_category.clone(),
            label: tr(locale, if paused { "Play" } else { "Pause" }).into_owned(),
            detail: None,
            keywords: Some(format!(
                "{} {}",
                tr(locale, if paused { "Pause" } else { "Play" }),
                tr(locale, "Play or pause")
            )),
            shortcut: Some(tr(locale, "Space / k").into_owned()),
            icon: if paused { Icon::Play } else { Icon::Pause },
            actions: vec![Action::TogglePause],
            search_query: None,
        });
        let muted = status.is_some_and(|s| s.muted);
        items.push(PaletteItem {
            category: player_category.clone(),
            label: tr(locale, if muted { "Unmute" } else { "Mute" }).into_owned(),
            detail: None,
            keywords: Some(tr(locale, if muted { "Mute" } else { "Unmute" }).into_owned()),
            shortcut: Some("m".into()),
            icon: if muted {
                Icon::VolumeMuted
            } else {
                Icon::Volume
            },
            actions: vec![Action::ToggleMute],
            search_query: None,
        });
        items.push(PaletteItem {
            category: player_category.clone(),
            label: tr(locale, "Theater mode").into_owned(),
            detail: None,
            keywords: None,
            shortcut: Some("t".into()),
            icon: Icon::Theater,
            actions: vec![Action::ToggleTheater],
            search_query: None,
        });
        items.push(PaletteItem {
            category: player_category.clone(),
            label: tr(
                locale,
                if state.fullscreen {
                    "Exit full screen"
                } else {
                    "Full screen"
                },
            )
            .into_owned(),
            detail: None,
            keywords: Some(tr(locale, "Full screen").into_owned()),
            shortcut: Some("f".into()),
            icon: if state.fullscreen {
                Icon::ExitFullscreen
            } else {
                Icon::Fullscreen
            },
            actions: vec![Action::ToggleFullscreen],
            search_query: None,
        });
        items.push(PaletteItem {
            category: player_category.clone(),
            label: tr(locale, "Mini player").into_owned(),
            detail: None,
            keywords: Some(tr(locale, "Miniplayer").into_owned()),
            shortcut: Some("i".into()),
            icon: Icon::MiniPlayer,
            actions: vec![Action::ToggleMiniPlayer],
            search_query: None,
        });
        items.push(PaletteItem {
            category: player_category.clone(),
            label: tr(locale, "Captions").into_owned(),
            detail: None,
            keywords: None,
            shortcut: Some("c".into()),
            icon: Icon::Captions,
            actions: vec![Action::ToggleCaptions],
            search_query: None,
        });
        items.push(PaletteItem {
            category: player_category.clone(),
            label: tr(locale, "Audio only").into_owned(),
            detail: None,
            keywords: Some(tr(locale, "Audio-only mode").into_owned()),
            shortcut: Some("Shift+A".into()),
            icon: Icon::AudioOnly,
            actions: vec![Action::ToggleAudioOnly],
            search_query: None,
        });
        items.push(PaletteItem {
            category: player_category.clone(),
            label: tr(locale, "Transcript").into_owned(),
            detail: None,
            keywords: Some(
                tr(
                    locale,
                    if state.transcript_open {
                        "Hide transcript"
                    } else {
                        "Show transcript"
                    },
                )
                .into_owned(),
            ),
            shortcut: None,
            icon: Icon::Captions,
            actions: vec![Action::ToggleTranscript],
            search_query: None,
        });
        items.push(PaletteItem {
            category: player_category.clone(),
            label: tr(locale, "Loop").into_owned(),
            detail: None,
            keywords: Some(tr(locale, "Loop video").into_owned()),
            shortcut: None,
            icon: Icon::Refresh,
            actions: vec![Action::ToggleLoop],
            search_query: None,
        });
        items.push(PaletteItem {
            category: player_category.clone(),
            label: tr(locale, "Next video").into_owned(),
            detail: None,
            keywords: None,
            shortcut: Some("Shift+N".into()),
            icon: Icon::SkipForward,
            actions: vec![Action::PlayNext],
            search_query: None,
        });
        items.push(PaletteItem {
            category: player_category,
            label: tr(locale, "Previous video").into_owned(),
            detail: None,
            keywords: None,
            shortcut: Some("Shift+P".into()),
            icon: Icon::Back,
            actions: vec![Action::PlayPrevious],
            search_query: None,
        });
    }

    // 3. Settings groups
    let settings_category = tr(locale, "Settings").into_owned();
    for group in SettingsGroup::ALL {
        items.push(PaletteItem {
            category: settings_category.clone(),
            label: tr(locale, group.label()).into_owned(),
            detail: None,
            keywords: None,
            shortcut: None,
            icon: Icon::Settings,
            actions: vec![
                Action::Navigate(View::Settings),
                Action::SetSettingsGroup(group),
            ],
            search_query: None,
        });
    }

    // 4. Recent searches
    let search_category = tr(locale, "Recent searches").into_owned();
    for query in state.recent_searches.iter().take(RECENT_SEARCHES_LIMIT) {
        let trimmed = query.trim();
        if trimmed.is_empty() {
            continue;
        }
        items.push(PaletteItem {
            category: search_category.clone(),
            label: trimmed.to_owned(),
            detail: None,
            keywords: None,
            shortcut: None,
            icon: Icon::Search,
            actions: vec![Action::Search(trimmed.to_owned())],
            search_query: Some(trimmed.to_owned()),
        });
    }

    // 5. Recent history entries
    let history_category = tr(locale, "History").into_owned();
    for watched in state.library.history.iter().take(RECENT_HISTORY_LIMIT) {
        let label = if watched.video.title.trim().is_empty() {
            watched.video.id.clone()
        } else {
            watched.video.title.clone()
        };
        let detail = watched
            .video
            .channel
            .as_ref()
            .map(|c| c.trim())
            .filter(|c| !c.is_empty())
            .map(str::to_owned);
        items.push(PaletteItem {
            category: history_category.clone(),
            label,
            detail,
            keywords: None,
            shortcut: None,
            icon: Icon::History,
            actions: vec![Action::Open(watched.video.clone())],
            search_query: None,
        });
    }

    items
}

fn is_word_boundary(prev: char) -> bool {
    prev.is_whitespace() || matches!(prev, '-' | '_' | '/' | ':' | '·' | '→' | '(' | ')' | '.')
}

/// Dependency-free, case-insensitive fuzzy scorer over lowered strings.
///
/// Ranking tiers:
/// - Exact full match: `10_000`
/// - Exact prefix match: `7_500..=8_000`
/// - Word-boundary contiguous prefix match: `5_500..=6_000`
/// - Contiguous substring match: `3_500..=4_000`
/// - Subsequence fuzzy match: `100..=3_000`
pub fn fuzzy_score_lowered(needle: &str, haystack: &str) -> Option<i32> {
    if needle.is_empty() {
        return Some(0);
    }
    if haystack == needle {
        return Some(10_000);
    }
    let n_chars: Vec<char> = needle.chars().collect();
    let h_chars: Vec<char> = haystack.chars().collect();
    if n_chars.len() > h_chars.len() {
        return None;
    }

    if haystack.starts_with(needle) {
        let extra = (h_chars.len() - n_chars.len()).min(500) as i32;
        return Some(8_000 - extra);
    }

    // Check contiguous character slice match (word-boundary first, then any substring).
    let n_len = n_chars.len();
    let mut first_substring_pos = None;
    for start in 0..=(h_chars.len() - n_len) {
        if h_chars[start..start + n_len] == n_chars[..] {
            if start == 0 || is_word_boundary(h_chars[start - 1]) {
                return Some(6_000 - (start.min(500) as i32));
            }
            if first_substring_pos.is_none() {
                first_substring_pos = Some(start);
            }
        }
    }
    if let Some(pos) = first_substring_pos {
        return Some(4_000 - (pos.min(500) as i32));
    }

    // Subsequence fuzzy match.
    let mut n_idx = 0;
    let mut first_match = 0;
    let mut prev_match = None;
    let mut bonus = 0i32;

    for (h_idx, &ch) in h_chars.iter().enumerate() {
        if ch == n_chars[n_idx] {
            if n_idx == 0 {
                first_match = h_idx;
            }
            if h_idx == 0 {
                bonus += 50;
            } else if is_word_boundary(h_chars[h_idx - 1]) {
                bonus += 35;
            }
            if prev_match == Some(h_idx.saturating_sub(1)) {
                bonus += 40;
            }
            prev_match = Some(h_idx);
            n_idx += 1;
            if n_idx == n_len {
                break;
            }
        }
    }

    if n_idx < n_len {
        return None;
    }

    let last_match = prev_match.unwrap_or(first_match);
    let span = (last_match - first_match + 1).saturating_sub(n_len) as i32;
    let raw = 1_500 + bonus - span * 4 - (first_match.min(300) as i32);
    Some(raw.clamp(100, 3_000))
}

/// Scores a query against a [`PaletteItem`].
pub fn score_item(query: &str, item: &PaletteItem) -> Option<i32> {
    let q = query.trim().to_lowercase();
    if q.is_empty() {
        return Some(0);
    }

    let label = item.label.to_lowercase();
    let category = item.category.to_lowercase();
    let detail = item.detail.as_deref().unwrap_or_default().to_lowercase();
    let keywords = item.keywords.as_deref().unwrap_or_default().to_lowercase();

    // Primary label match gets a priority boost so direct label matches rank above category-only matches.
    let mut best = fuzzy_score_lowered(&q, &label).map(|s| s + 300);

    if !detail.is_empty()
        && let Some(s) = fuzzy_score_lowered(&q, &detail).map(|s| s + 100)
    {
        best = Some(best.map_or(s, |b| b.max(s)));
    }

    if !keywords.is_empty()
        && let Some(s) = fuzzy_score_lowered(&q, &keywords).map(|s| s + 150)
    {
        best = Some(best.map_or(s, |b| b.max(s)));
    }

    let combined = if detail.is_empty() && keywords.is_empty() {
        format!("{category} {label}")
    } else if keywords.is_empty() {
        format!("{category} {label} {detail}")
    } else {
        format!("{category} {label} {detail} {keywords}")
    };

    if let Some(s) = fuzzy_score_lowered(&q, &combined) {
        best = Some(best.map_or(s, |b| b.max(s)));
    }

    // Multi-word queries: if every token matches the combined item text, score the token sum/average.
    let tokens: Vec<&str> = q.split_whitespace().collect();
    if tokens.len() > 1 {
        let mut sum = 0i32;
        let mut all_match = true;
        for token in &tokens {
            let token_best = fuzzy_score_lowered(token, &label)
                .map(|s| s + 200)
                .into_iter()
                .chain(fuzzy_score_lowered(token, &detail).map(|s| s + 100))
                .chain(fuzzy_score_lowered(token, &combined))
                .max();
            if let Some(s) = token_best {
                sum += s;
            } else {
                all_match = false;
                break;
            }
        }
        if all_match {
            let avg = sum / (tokens.len() as i32);
            best = Some(best.map_or(avg, |b| b.max(avg)));
        }
    }

    best
}

/// Returns the candidate items matching `state.palette_query`, ranked by fuzzy score
/// descending and preserving logical category/recency order on ties.
pub fn matching_items(state: &State, status: Option<&Status>) -> Vec<PaletteItem> {
    let candidates = candidate_items(state, status);
    let query = state.palette_query.trim();
    if query.is_empty() {
        return candidates;
    }
    let mut scored: Vec<(i32, usize, PaletteItem)> = candidates
        .into_iter()
        .enumerate()
        .filter_map(|(index, item)| score_item(query, &item).map(|score| (score, index, item)))
        .collect();
    scored.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(&b.1)));
    scored.into_iter().map(|(_, _, item)| item).collect()
}

fn execute_item(
    ctx: &egui::Context,
    state: &mut State,
    item: &PaletteItem,
    actions: &mut Vec<Action>,
) {
    if let Some(query) = &item.search_query {
        state.query.clone_from(query);
    }
    actions.extend(item.actions.iter().cloned());
    close(ctx, state);
}

/// Processes keyboard navigation while the command palette is open.
///
/// Called from `keys::handle` (`before_text_edit = true`) so background shortcuts
/// never fire while the palette is open, and again after the search field in
/// [`show`] (`before_text_edit = false`) so text events and key presses in the
/// same frame are handled in order.
pub(super) fn handle_keys(
    ctx: &egui::Context,
    state: &mut State,
    status: Option<&Status>,
    actions: &mut Vec<Action>,
    before_text_edit: bool,
) {
    if !state.palette_open {
        return;
    }
    let escaped = ctx.input_mut(|input| input.consume_key(egui::Modifiers::NONE, Key::Escape));
    if escaped {
        close(ctx, state);
        return;
    }
    // If there is pending text input in this frame, let the search field update
    // `state.palette_query` first before evaluating cursor movement or Enter.
    if before_text_edit
        && ctx.input(|input| {
            input
                .events
                .iter()
                .any(|event| matches!(event, egui::Event::Text(_)))
        })
    {
        return;
    }
    let (down, up, enter) = ctx.input_mut(|input| {
        let down = input.count_and_consume_key(egui::Modifiers::NONE, Key::ArrowDown)
            + input.count_and_consume_key(egui::Modifiers::NONE, Key::Tab);
        let up = input.count_and_consume_key(egui::Modifiers::NONE, Key::ArrowUp)
            + input.count_and_consume_key(egui::Modifiers::SHIFT, Key::Tab);
        let enter = input.consume_key(egui::Modifiers::NONE, Key::Enter);
        (down, up, enter)
    });
    if down == 0 && up == 0 && !enter {
        return;
    }
    let items = matching_items(state, status);
    let count = items.len();
    if count == 0 {
        state.palette_cursor = 0;
        return;
    }
    state.palette_cursor = state.palette_cursor.min(count - 1);
    if down > 0 {
        state.palette_cursor = (state.palette_cursor + down).min(count - 1);
        mark_cursor_moved(ctx);
    }
    if up > 0 {
        state.palette_cursor = state.palette_cursor.saturating_sub(up);
        mark_cursor_moved(ctx);
    }
    if enter {
        execute_item(ctx, state, &items[state.palette_cursor], actions);
    }
}

/// Renders the command palette modal overlay when `state.palette_open` is `true`.
pub fn show(
    ctx: &egui::Context,
    state: &mut State,
    status: Option<&Status>,
    actions: &mut Vec<Action>,
) {
    if !state.palette_open {
        ctx.data_mut(|data| {
            data.insert_temp(was_open_id(), false);
            data.insert_temp(focus_pending_id(), false);
        });
        return;
    }
    let focus_input = ctx.data_mut(|data| {
        let was_open = data.get_temp::<bool>(was_open_id()).unwrap_or(false);
        let pending = data.get_temp::<bool>(focus_pending_id()).unwrap_or(false);
        data.insert_temp(was_open_id(), true);
        data.insert_temp(focus_pending_id(), false);
        !was_open || pending
    });

    let locale = state.settings.language;
    let palette = state.palette;
    let raised = widgets::on_card(&palette);
    let id = egui::Id::new("command-palette");
    let width = (ctx.content_rect().width() - 48.0).clamp(260.0, 560.0);
    let max_list_height = (ctx.content_rect().height() - 180.0).clamp(140.0, 380.0);

    let mut chosen: Option<PaletteItem> = None;
    let response = egui::Modal::new(id)
        .frame(
            egui::Frame::popup(&ctx.style_of(ctx.theme()))
                .fill(palette.overlay)
                .stroke(Stroke::new(1.0, palette.outline))
                .corner_radius(CornerRadius::same(RADIUS + 4))
                .inner_margin(egui::Margin::same(14)),
        )
        .show(ctx, |ui| {
            ui.set_width(width);
            let field = widgets::search_field(
                ui,
                &raised,
                &mut state.palette_query,
                tr(locale, "Type a command or search…").as_ref(),
                locale,
                input_id(),
                width,
            );
            if focus_input {
                field.request_focus();
            }
            if field.changed() {
                state.palette_cursor = 0;
                mark_cursor_moved(ui.ctx());
            }
            handle_keys(ui.ctx(), state, status, actions, false);
            if !state.palette_open {
                return;
            }
            let items = matching_items(state, status);
            if items.is_empty() {
                state.palette_cursor = 0;
                ui.add_space(16.0);
                ui.vertical_centered(|ui| {
                    ui.label(
                        egui::RichText::new(tr(locale, "No matching commands").as_ref())
                            .font(theme::regular(14.0))
                            .color(palette.secondary),
                    );
                });
                ui.add_space(12.0);
                return;
            }
            state.palette_cursor = state.palette_cursor.min(items.len() - 1);
            let cursor_moved = take_cursor_moved(ui.ctx());
            ui.add_space(8.0);
            egui::ScrollArea::vertical()
                .id_salt("command-palette-list")
                .max_height(max_list_height)
                .auto_shrink([false, true])
                .show(ui, |ui| {
                    ui.spacing_mut().item_spacing.y = 2.0;
                    let row_width = ui.available_width().max(120.0);
                    for (index, item) in items.iter().enumerate() {
                        let selected = index == state.palette_cursor;
                        let response = draw_row(ui, &palette, &raised, item, selected, row_width);
                        if selected && cursor_moved {
                            response.scroll_to_me(None);
                        }
                        if response.clicked() {
                            state.palette_cursor = index;
                            chosen = Some(item.clone());
                        }
                    }
                });
        });

    if let Some(item) = chosen {
        execute_item(ctx, state, &item, actions);
    } else if state.palette_open && response.should_close() {
        close(ctx, state);
    }
}

fn draw_row(
    ui: &mut Ui,
    palette: &youtfast_appearance::theme::Palette,
    raised: &youtfast_appearance::theme::Palette,
    item: &PaletteItem,
    selected: bool,
    width: f32,
) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(vec2(width, ROW_HEIGHT), Sense::click());
    response.widget_info(|| {
        egui::WidgetInfo::selected(egui::WidgetType::Button, true, selected, &item.label)
    });
    if !ui.is_rect_visible(rect) {
        return response;
    }
    if response.hovered() {
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    }
    let radius = CornerRadius::same(theme::RADIUS_SMALL + 2);
    widgets::row_fill_radius(ui, palette, rect, &response, selected, radius);
    if selected {
        let bar = Rect::from_center_size(pos2(rect.left() + 4.0, rect.center().y), vec2(3.0, 18.0));
        ui.painter()
            .rect_filled(bar, CornerRadius::same(2), palette.accent);
    }

    let icon_color = if selected {
        palette.text
    } else {
        palette.secondary
    };
    let icon_rect =
        Rect::from_center_size(pos2(rect.left() + 20.0, rect.center().y), vec2(16.0, 16.0));
    item.icon.image(icon_color, 16.0).paint_at(ui, icon_rect);

    let mut right = rect.right() - 10.0;
    if let Some(shortcut) = &item.shortcut {
        let galley =
            ui.painter()
                .layout_no_wrap(shortcut.clone(), theme::medium(12.0), palette.secondary);
        let pill_w = (galley.size().x + 12.0).max(22.0);
        let pill_h = 22.0;
        let pill_rect = Rect::from_center_size(
            pos2(right - pill_w / 2.0, rect.center().y),
            vec2(pill_w, pill_h),
        );
        ui.painter()
            .rect_filled(pill_rect, CornerRadius::same(11), raised.surface);
        ui.painter().galley(
            pill_rect.center() - galley.size() / 2.0,
            galley,
            palette.secondary,
        );
        right = pill_rect.left() - 8.0;
    }

    let cat_galley = ui.painter().layout_no_wrap(
        item.category.clone(),
        theme::medium(12.0),
        palette.secondary,
    );
    let cat_pos = pos2(
        right - cat_galley.size().x,
        rect.center().y - cat_galley.size().y / 2.0,
    );
    ui.painter().galley(cat_pos, cat_galley, palette.secondary);
    right = (cat_pos.x - 12.0).max(icon_rect.right() + 24.0);

    let text_left = icon_rect.right() + 10.0;
    let max_text_width = (right - text_left).max(16.0);
    let label_font = if selected {
        theme::semibold(14.0)
    } else {
        theme::medium(14.0)
    };
    let mut label_job = LayoutJob::default();
    label_job.append(
        &item.label,
        0.0,
        egui::TextFormat::simple(label_font, palette.text),
    );
    label_job.wrap.max_width = max_text_width;
    label_job.wrap.max_rows = 1;
    label_job.wrap.overflow_character = Some('…');
    let label_galley = ui.painter().layout_job(label_job);
    let label_width = label_galley.size().x;
    ui.painter().galley(
        pos2(text_left, rect.center().y - label_galley.size().y / 2.0),
        label_galley,
        palette.text,
    );

    if let Some(detail) = &item.detail {
        let detail_left = text_left + label_width + 8.0;
        let detail_room = right - detail_left;
        if detail_room >= 28.0 {
            let mut detail_job = LayoutJob::default();
            detail_job.append(
                detail,
                0.0,
                egui::TextFormat::simple(theme::regular(13.0), palette.secondary),
            );
            detail_job.wrap.max_width = detail_room;
            detail_job.wrap.max_rows = 1;
            detail_job.wrap.overflow_character = Some('…');
            let detail_galley = ui.painter().layout_job(detail_job);
            ui.painter().galley(
                pos2(detail_left, rect.center().y - detail_galley.size().y / 2.0),
                detail_galley,
                palette.secondary,
            );
        }
    }

    response
}

#[cfg(test)]
mod tests {
    use super::*;
    use youtfast_core::model::Video;
    use youtfast_i18n::Locale;

    fn sample_state() -> State {
        let mut state = State {
            watching: Some(Video {
                title: "Rust Async Under the Hood".into(),
                channel: Some("Jon Gjengset".into()),
                ..Video::from_id("vid00000001".into())
            }),
            can_go_back: true,
            can_go_forward: true,
            ..State::default()
        };
        state.record_recent_search("egui layout");
        state.record_recent_search("libmpv shaders");
        state.library.record_watch(
            Video {
                title: "Why Branch Prediction Changed Everything".into(),
                channel: Some("Computerphile".into()),
                ..Video::from_id("hist0000001".into())
            },
            100,
        );
        state.library.record_watch(
            Video {
                title: "How Linux Memory Allocation Actually Works".into(),
                channel: Some("Fabio Akita".into()),
                ..Video::from_id("hist0000002".into())
            },
            200,
        );
        state
    }

    #[test]
    fn candidates_include_views_actions_settings_recent_searches_and_history() {
        let mut state = sample_state();
        let status = Status {
            paused: true,
            muted: false,
            ..Status::default()
        };
        let items = candidate_items(&state, Some(&status));

        // Signed-out views: Liked videos and Playlists are gated on sign-in.
        assert!(
            items
                .iter()
                .any(|i| i.actions == [Action::Navigate(View::Home)])
        );
        assert!(
            items
                .iter()
                .any(|i| i.actions == [Action::Navigate(View::Subscriptions)])
        );
        assert!(
            !items
                .iter()
                .any(|i| i.actions == [Action::Navigate(View::Liked)])
        );
        assert!(
            !items
                .iter()
                .any(|i| i.actions == [Action::Navigate(View::Playlists)])
        );

        // Signed-in views include Liked videos and Playlists.
        state.account = AccountState::SignedIn(1);
        let signed_in_items = candidate_items(&state, Some(&status));
        assert!(
            signed_in_items
                .iter()
                .any(|i| i.actions == [Action::Navigate(View::Liked)])
        );
        assert!(
            signed_in_items
                .iter()
                .any(|i| i.actions == [Action::Navigate(View::Playlists)])
        );

        // Page and player actions.
        for expected in [
            Action::Refresh,
            Action::ToggleSidebar,
            Action::Back,
            Action::Forward,
            Action::NewVideoTab,
            Action::TogglePause,
            Action::ToggleMute,
            Action::ToggleTheater,
            Action::ToggleFullscreen,
            Action::ToggleMiniPlayer,
            Action::ToggleCaptions,
            Action::ToggleAudioOnly,
            Action::ToggleTranscript,
            Action::ToggleLoop,
            Action::PlayNext,
            Action::PlayPrevious,
        ] {
            assert!(
                items.iter().any(|i| i.actions == [expected.clone()]),
                "missing action {expected:?}"
            );
        }

        // Settings groups.
        for group in SettingsGroup::ALL {
            assert!(
                items.iter().any(|i| {
                    i.actions
                        == [
                            Action::Navigate(View::Settings),
                            Action::SetSettingsGroup(group),
                        ]
                }),
                "missing settings group {group:?}"
            );
        }

        // Recent searches (most recent first).
        let searches: Vec<&str> = items
            .iter()
            .filter_map(|i| i.search_query.as_deref())
            .collect();
        assert_eq!(searches, ["libmpv shaders", "egui layout"]);

        // Recent history entries (most recent first).
        assert!(items.iter().any(|i| {
            i.label == "How Linux Memory Allocation Actually Works"
                && i.detail.as_deref() == Some("Fabio Akita")
                && matches!(i.actions.as_slice(), [Action::Open(v)] if v.id == "hist0000002")
        }));
    }

    #[test]
    fn fuzzy_matcher_ranks_prefix_above_word_boundary_above_subsequence() {
        let prefix = fuzzy_score_lowered("thea", "theater mode").unwrap();
        let word_boundary = fuzzy_score_lowered("mode", "theater mode").unwrap();
        let substring = fuzzy_score_lowered("eate", "theater mode").unwrap();
        let subsequence = fuzzy_score_lowered("thmd", "theater mode").unwrap();

        assert!(prefix > word_boundary, "{prefix} vs {word_boundary}");
        assert!(word_boundary > substring, "{word_boundary} vs {substring}");
        assert!(substring > subsequence, "{substring} vs {subsequence}");
        assert!(fuzzy_score_lowered("xyz", "theater mode").is_none());
    }

    #[test]
    fn matching_items_finds_history_by_channel_and_settings_by_group() {
        let mut state = sample_state();
        state.palette_query = "computerphile".into();
        let matched = matching_items(&state, None);
        assert!(!matched.is_empty());
        assert_eq!(matched[0].label, "Why Branch Prediction Changed Everything");

        state.palette_query = "theater".into();
        let matched = matching_items(&state, None);
        assert_eq!(matched[0].actions, [Action::ToggleTheater]);

        state.palette_query = "settings playback".into();
        let matched = matching_items(&state, None);
        assert_eq!(
            matched[0].actions,
            [
                Action::Navigate(View::Settings),
                Action::SetSettingsGroup(SettingsGroup::Playback),
            ]
        );
    }

    #[test]
    fn keyboard_arrows_enter_and_escape_navigate_and_execute_in_modal() {
        let ctx = egui::Context::default();
        theme::install_fonts(&ctx);
        let mut state = sample_state();
        open(&ctx, &mut state);
        assert!(state.palette_open);
        assert_eq!(state.palette_cursor, 0);

        let run_frame = |state: &mut State, events: Vec<egui::Event>| {
            let mut actions = Vec::new();
            let mut output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(1280.0, 800.0))),
                    events,
                    ..Default::default()
                },
                |ui| show(ui.ctx(), state, None, &mut actions),
            );
            output.textures_delta.clear();
            actions
        };

        // Initial render focuses the input.
        assert!(run_frame(&mut state, Vec::new()).is_empty());
        assert!(ctx.memory(|m| m.has_focus(input_id())));

        // ArrowDown moves the cursor to row 1 (Subscriptions), ArrowUp back to row 0 (Home).
        assert!(
            run_frame(
                &mut state,
                vec![egui::Event::Key {
                    key: Key::ArrowDown,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: egui::Modifiers::NONE,
                }],
            )
            .is_empty()
        );
        assert_eq!(state.palette_cursor, 1);

        // Filter to "libmpv shaders" and press Enter.
        state.palette_query = "libmpv shaders".into();
        state.palette_cursor = 0;
        let actions = run_frame(
            &mut state,
            vec![egui::Event::Key {
                key: Key::Enter,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            }],
        );
        assert!(!state.palette_open);
        assert_eq!(state.query, "libmpv shaders");
        assert_eq!(actions, [Action::Search("libmpv shaders".into())]);

        // Reopen and press Escape to close without actions.
        open(&ctx, &mut state);
        let actions = run_frame(
            &mut state,
            vec![egui::Event::Key {
                key: Key::Escape,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            }],
        );
        assert!(!state.palette_open);
        assert!(actions.is_empty());
    }

    #[test]
    fn command_palette_renders_localized_labels_and_empty_state_in_every_locale() {
        for &locale in Locale::ALL {
            let ctx = egui::Context::default();
            theme::install_fonts(&ctx);
            let mut state = sample_state();
            state.settings.language = locale;
            open(&ctx, &mut state);
            state.palette_query = "nonexistent_command_query_xyz".into();

            let mut texts = Vec::new();
            for _ in 0..2 {
                texts.clear();
                let mut output = ctx.run_ui(
                    egui::RawInput {
                        screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(1280.0, 800.0))),
                        ..Default::default()
                    },
                    |ui| show(ui.ctx(), &mut state, None, &mut Vec::new()),
                );
                for shape in &output.shapes {
                    if let egui::Shape::Text(text) = &shape.shape {
                        texts.push(text.galley.text().to_owned());
                    }
                }
                output.textures_delta.clear();
            }
            let expected_empty = tr(locale, "No matching commands");
            assert!(
                texts.iter().any(|t| t == expected_empty.as_ref()),
                "missing localized empty state {expected_empty:?} for {locale:?}; got {texts:?}"
            );
        }
    }

    #[test]
    fn recent_searches_deduplicate_case_insensitively_and_stay_bounded() {
        let mut state = State::default();
        for i in 0..(RECENT_SEARCHES_LIMIT + 5) {
            state.record_recent_search(&format!("query {i}"));
        }
        assert_eq!(state.recent_searches.len(), RECENT_SEARCHES_LIMIT);
        assert_eq!(
            state.recent_searches[0],
            format!("query {}", RECENT_SEARCHES_LIMIT + 4)
        );
        state.record_recent_search("  QUERY 10  ");
        assert_eq!(state.recent_searches.len(), RECENT_SEARCHES_LIMIT);
        assert_eq!(state.recent_searches[0], "QUERY 10");
        assert_eq!(
            state
                .recent_searches
                .iter()
                .filter(|q| q.eq_ignore_ascii_case("query 10"))
                .count(),
            1
        );
    }

    #[test]
    fn mouse_click_on_palette_row_executes_command_and_closes_modal() {
        let ctx = egui::Context::default();
        theme::install_fonts(&ctx);
        let mut state = sample_state();
        open(&ctx, &mut state);
        state.palette_query = "thea".into();

        let screen = Rect::from_min_size(pos2(0.0, 0.0), vec2(1280.0, 800.0));
        let mut row_pos = None;
        for _ in 0..2 {
            let mut output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(screen),
                    ..Default::default()
                },
                |ui| show(ui.ctx(), &mut state, None, &mut Vec::new()),
            );
            row_pos = output.shapes.iter().find_map(|shape| match &shape.shape {
                egui::Shape::Text(text) if text.galley.text() == "Theater mode" => {
                    Some(text.pos + text.galley.size() / 2.0)
                }
                _ => None,
            });
            output.textures_delta.clear();
        }
        let pos = row_pos.expect("Theater mode row rendered");
        let mut actions = Vec::new();
        for pressed in [true, false] {
            let mut output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(screen),
                    events: vec![
                        egui::Event::PointerMoved(pos),
                        egui::Event::PointerButton {
                            pos,
                            button: egui::PointerButton::Primary,
                            pressed,
                            modifiers: egui::Modifiers::NONE,
                        },
                    ],
                    ..Default::default()
                },
                |ui| show(ui.ctx(), &mut state, None, &mut actions),
            );
            output.textures_delta.clear();
        }
        assert!(!state.palette_open);
        assert_eq!(actions, [Action::ToggleTheater]);
    }
}
