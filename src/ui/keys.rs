//! Focus-aware shortcuts. Text fields and open menus own their keystrokes.
use super::*;

/// Points a single `ArrowUp` or `ArrowDown` press scrolls the page.
pub(super) const KEY_SCROLL_STEP: f32 = fastframe_scroll::WHEEL_STEP;

pub(super) fn handle(
    ctx: &egui::Context,
    state: &mut State,
    status: Option<&Status>,
    actions: &mut Vec<Action>,
) {
    if ctx.current_pass_index() == 0 {
        state.cursor_moved = false;
    }
    if ctx.input(|i| i.pointer.button_clicked(egui::PointerButton::Extra1)) {
        actions.push(Action::Back);
    }
    if ctx.input(|i| i.pointer.button_clicked(egui::PointerButton::Extra2)) {
        actions.push(Action::Forward);
    }
    if ctx.text_edit_focused() || egui::Popup::is_any_open(ctx) {
        return;
    }
    let focus = ctx.memory(|m| m.focused());
    let focused = focus.is_some();
    let player_focused = focus.is_some_and(|id| {
        ctx.data(|d| d.get_temp::<egui::Id>(egui::Id::new("video-keyboard-focus"))) == Some(id)
            || ctx.data(|d| d.get_temp::<egui::Id>(egui::Id::new("timeline-keyboard-focus")))
                == Some(id)
    });
    let controls_focused = focused && !player_focused;
    let search_shortcut = !state.keys_open
        && state.player_menu == crate::app::PlayerMenu::Closed
        && ctx.input(|input| {
            input.key_pressed(Key::Slash)
                && !input.modifiers.ctrl
                && !input.modifiers.alt
                && !input.modifiers.command
        });
    if search_shortcut {
        ctx.input_mut(|input| {
            input.consume_key(egui::Modifiers::NONE, Key::Slash);
            input
                .events
                .retain(|e| !matches!(e,egui::Event::Text(text) if text=="/"));
        });
        ctx.memory_mut(|memory| memory.request_focus(command_line_id()));
        return;
    }
    ctx.input_mut(|input| {
        if input.key_pressed(Key::Questionmark) {
            state.keys_open = !state.keys_open;
            return;
        }
        if state.keys_open {
            if input.consume_key(egui::Modifiers::NONE, Key::Escape) {
                state.keys_open = false;
            }
            return;
        }
        if input.modifiers.alt && input.key_pressed(Key::ArrowLeft) && !cfg!(target_os = "macos") {
            actions.push(Action::Back);
            return;
        }
        if input.modifiers.alt && input.key_pressed(Key::ArrowRight) && !cfg!(target_os = "macos") {
            actions.push(Action::Forward);
            return;
        }
        if state.player_menu != crate::app::PlayerMenu::Closed {
            if input.consume_key(egui::Modifiers::NONE, Key::Escape) {
                state.player_menu = crate::app::PlayerMenu::Closed;
            }
            return;
        }
        let chapter_modifier = if cfg!(target_os = "macos") {
            input.modifiers.alt
        } else {
            input.modifiers.ctrl
        };
        if state.view == View::Watch && chapter_modifier && !controls_focused {
            if input.key_pressed(Key::ArrowLeft) {
                actions.push(Action::SeekChapter(false));
            }
            if input.key_pressed(Key::ArrowRight) {
                actions.push(Action::SeekChapter(true));
            }
            return;
        }
        if input.modifiers.ctrl
            || input.modifiers.alt
            || input.modifiers.command
            || input.modifiers.mac_cmd
        {
            return;
        }
        if input.key_pressed(Key::R) {
            actions.push(Action::Refresh);
        }
        if input.key_pressed(Key::B) {
            actions.push(Action::ToggleSidebar);
        }
        let playing = state.watching.is_some() && !state.mini_closed;
        if input.key_pressed(Key::I) && playing {
            actions.push(Action::ToggleMiniPlayer);
            return;
        }
        // Unambiguous playback commands also work while browsing with the mini player.
        if playing || state.view == View::Watch {
            if input.key_pressed(Key::M) {
                actions.push(Action::ToggleMute);
            }
            if input.key_pressed(Key::C) {
                actions.push(Action::ToggleCaptions);
            }
            if (!focused || player_focused) && input.consume_key(egui::Modifiers::NONE, Key::Space)
            {
                actions.push(Action::TogglePause);
            }
            if input.modifiers.shift && input.key_pressed(Key::N) {
                actions.push(Action::PlayNext);
            }
            if input.modifiers.shift && input.key_pressed(Key::P) {
                actions.push(Action::PlayPrevious);
            }
            if input.modifiers.shift && input.key_pressed(Key::A) {
                actions.push(Action::ToggleAudioOnly);
            }
        }
        if state.view != View::Watch {
            // The number keys follow the buttons as the sidebar shows them.
            let views = sidebar_views(state);
            let digits = [
                Key::Num1,
                Key::Num2,
                Key::Num3,
                Key::Num4,
                Key::Num5,
                Key::Num6,
                Key::Num7,
                Key::Num8,
                Key::Num9,
            ];
            if let Some(index) = digits.iter().position(|key| input.key_pressed(*key))
                && let Some(view) = views.get(index)
            {
                actions.push(Action::Navigate(*view));
            }
            if !focused {
                let down = input.count_and_consume_key(egui::Modifiers::NONE, Key::ArrowDown);
                let up = input.count_and_consume_key(egui::Modifiers::NONE, Key::ArrowUp);
                if down > 0 || up > 0 {
                    state.page_scroll += (up as f32 - down as f32) * KEY_SCROLL_STEP;
                }
            }
            let count = cursor_count(state);
            if count > 0 {
                if input.key_pressed(Key::J) {
                    state.cursor = (state.cursor + 1).min(count - 1);
                    state.cursor_moved = true;
                }
                if input.key_pressed(Key::K) {
                    state.cursor = state.cursor.saturating_sub(1);
                    state.cursor_moved = true;
                }
                if input.key_pressed(Key::G) {
                    state.cursor = if input.modifiers.shift { count - 1 } else { 0 };
                    state.cursor_moved = true;
                }
                if !focused
                    && input.key_pressed(Key::Enter)
                    && let Some(action) = open_at(state, state.cursor)
                {
                    actions.push(action);
                }
            }
            if state.view == View::Channel && !focused {
                let tabs = channel::tabs(state.channel_header.as_ref(), &state.channel_tab);
                if input.key_pressed(Key::H) {
                    actions.push(Action::SetChannelTab(channel::cycle_tab(
                        &tabs,
                        &state.channel_tab,
                        -1,
                    )));
                }
                if input.key_pressed(Key::L) {
                    actions.push(Action::SetChannelTab(channel::cycle_tab(
                        &tabs,
                        &state.channel_tab,
                        1,
                    )));
                }
            }
            if !input.modifiers.shift
                && input.key_pressed(Key::A)
                && state.library.accounts.len() > 1
            {
                let accounts = &state.library.accounts;
                let at = accounts
                    .iter()
                    .position(|a| Some(a.id) == state.library.active_account)
                    .map_or(0, |i| i + 1)
                    % accounts.len();
                actions.push(Action::SwitchAccount(accounts[at].id));
            }
            if input.consume_key(egui::Modifiers::NONE, Key::Escape) {
                actions.push(if playing {
                    Action::CloseMiniPlayer
                } else {
                    Action::Back
                });
            }
            return;
        }
        if input.key_pressed(Key::K) {
            actions.push(Action::TogglePause);
        }
        for (index, key) in [
            Key::Num0,
            Key::Num1,
            Key::Num2,
            Key::Num3,
            Key::Num4,
            Key::Num5,
            Key::Num6,
            Key::Num7,
            Key::Num8,
            Key::Num9,
        ]
        .iter()
        .enumerate()
        {
            if input.key_pressed(*key) {
                actions.push(Action::SeekFraction(index as f64 / 10.0));
            }
        }
        if !controls_focused && input.consume_key(input.modifiers, Key::ArrowLeft) {
            actions.push(Action::SeekBy(-5.0));
        }
        if !controls_focused && input.consume_key(input.modifiers, Key::ArrowRight) {
            actions.push(Action::SeekBy(5.0));
        }
        if input.key_pressed(Key::J) {
            actions.push(Action::SeekBy(-10.0));
        }
        if input.key_pressed(Key::L) {
            actions.push(Action::SeekBy(10.0));
        }
        if !controls_focused && input.consume_key(input.modifiers, Key::Home) {
            actions.push(Action::SeekFraction(0.0));
        }
        if !controls_focused
            && input.consume_key(input.modifiers, Key::End)
            && let Some(s) = status
        {
            actions.push(Action::SeekTo {
                seconds: (s.duration - 0.1).max(0.0),
                exact: true,
            });
        }
        if !controls_focused && input.consume_key(input.modifiers, Key::ArrowUp) {
            actions.push(if state.watching_short() {
                Action::PlayPrevious
            } else {
                Action::SetVolume((status.map_or(100.0, |s| s.volume) + 5.0).min(100.0))
            });
        }
        if !controls_focused && input.consume_key(input.modifiers, Key::ArrowDown) {
            actions.push(if state.watching_short() {
                Action::PlayNext
            } else {
                Action::SetVolume((status.map_or(100.0, |s| s.volume) - 5.0).max(0.0))
            });
        }
        let speed = status.map_or(1.0, |s| s.speed);
        for event in &input.events {
            if let egui::Event::Text(text) = event {
                match text.as_str() {
                    "<" => actions.push(Action::SetSpeed(crate::app::player::step_speed(
                        speed, false,
                    ))),
                    ">" => actions.push(Action::SetSpeed(crate::app::player::step_speed(
                        speed, true,
                    ))),
                    "," => actions.push(Action::FrameStep(false)),
                    "." => actions.push(Action::FrameStep(true)),
                    _ => {}
                }
            }
        }
        if input.key_pressed(Key::F) {
            actions.push(Action::ToggleFullscreen);
        }
        if input.key_pressed(Key::T) && !state.fullscreen {
            actions.push(Action::ToggleTheater);
        }
        if input.consume_key(egui::Modifiers::NONE, Key::Escape) {
            actions.push(if state.autoplay.is_some() {
                Action::CancelAutoplay
            } else if state.fullscreen {
                Action::ExitFullscreen
            } else {
                Action::Back
            });
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn focused_volume_slider_owns_arrows_and_focused_video_space_pauses_once() {
        for video_focus in [false, true] {
            let ctx = egui::Context::default();
            let id = egui::Id::new("focused-control");
            ctx.memory_mut(|m| m.request_focus(id));
            if video_focus {
                ctx.data_mut(|d| d.insert_temp(egui::Id::new("video-keyboard-focus"), id));
            }
            let mut state = State {
                view: View::Watch,
                watching: Some(crate::model::Video::from_id("aaaaaaaaaaa".into())),
                ..State::default()
            };
            let mut actions = Vec::new();
            let key = if video_focus {
                Key::Space
            } else {
                Key::ArrowRight
            };
            let mut out = ctx.run_ui(
                egui::RawInput {
                    events: vec![egui::Event::Key {
                        key,
                        physical_key: None,
                        pressed: true,
                        repeat: false,
                        modifiers: egui::Modifiers::NONE,
                    }],
                    ..Default::default()
                },
                |ui| {
                    handle(ui.ctx(), &mut state, Some(&Status::default()), &mut actions);
                    let response = ui.interact(ui.max_rect(), id, Sense::click());
                    if response.clicked() {
                        actions.push(Action::TogglePause);
                    }
                },
            );
            out.textures_delta.clear();
            assert_eq!(
                actions,
                if video_focus {
                    vec![Action::TogglePause]
                } else {
                    Vec::new()
                }
            );
        }
    }
    #[test]
    fn search_shortcut_focuses_without_inserting_a_slash_or_locking_the_context() {
        let ctx = egui::Context::default();
        let mut state = State::default();
        let mut actions = Vec::new();
        let mut out = ctx.run_ui(
            egui::RawInput {
                events: vec![
                    egui::Event::Key {
                        key: Key::Slash,
                        physical_key: None,
                        pressed: true,
                        repeat: false,
                        modifiers: egui::Modifiers::NONE,
                    },
                    egui::Event::Text("/".into()),
                ],
                ..Default::default()
            },
            |ui| {
                handle(ui.ctx(), &mut state, None, &mut actions);
                ui.add(egui::TextEdit::singleline(&mut state.query).id(command_line_id()));
            },
        );
        out.textures_delta.clear();
        assert!(state.query.is_empty());
        assert!(ctx.memory(|m| m.has_focus(command_line_id())));
        assert!(actions.is_empty());
    }
    fn press(state: &mut State, key: Key, modifiers: egui::Modifiers) -> Vec<Action> {
        let ctx = egui::Context::default();
        let mut actions = Vec::new();
        let mut output = ctx.run_ui(
            egui::RawInput {
                events: vec![
                    egui::Event::ModifiersChanged(modifiers),
                    egui::Event::Key {
                        key,
                        physical_key: None,
                        pressed: true,
                        repeat: false,
                        modifiers,
                    },
                ],
                ..Default::default()
            },
            |ui| handle(ui.ctx(), state, Some(&Status::default()), &mut actions),
        );
        output.textures_delta.clear();
        actions
    }
    #[test]
    fn chapter_modifiers_do_not_also_seek_five_seconds() {
        let mut state = State {
            view: View::Watch,
            ..State::default()
        };
        let modifiers = if cfg!(target_os = "macos") {
            egui::Modifiers::ALT
        } else {
            egui::Modifiers::CTRL
        };
        assert_eq!(
            press(&mut state, Key::ArrowRight, modifiers),
            vec![Action::SeekChapter(true)]
        );
        assert!(press(&mut state, Key::C, egui::Modifiers::CTRL).is_empty());
    }
    #[test]
    fn mini_player_controls_do_not_steal_the_lists_k_binding() {
        let mut state = State {
            watching: Some(crate::model::Video::from_id("aaaaaaaaaaa".into())),
            ..State::default()
        };
        assert_eq!(
            press(&mut state, Key::Space, egui::Modifiers::NONE),
            vec![Action::TogglePause]
        );
        assert_eq!(
            press(&mut state, Key::I, egui::Modifiers::NONE),
            vec![Action::ToggleMiniPlayer]
        );
        assert!(press(&mut state, Key::K, egui::Modifiers::NONE).is_empty());
    }
    #[test]
    fn shift_a_toggles_audio_only_while_playing() {
        let mut state = State {
            watching: Some(crate::model::Video::from_id("aaaaaaaaaaa".into())),
            ..State::default()
        };
        assert_eq!(
            press(&mut state, Key::A, egui::Modifiers::SHIFT),
            vec![Action::ToggleAudioOnly]
        );
    }
    #[test]
    fn shift_a_does_not_also_switch_accounts_while_browsing() {
        let mut state = State {
            watching: Some(crate::model::Video::from_id("aaaaaaaaaaa".into())),
            ..State::default()
        };
        for id in [1, 2] {
            state
                .library
                .accounts
                .push(crate::storage::library::Account {
                    id,
                    name: format!("account {id}"),
                    source: crate::model::AccountSource::Browser("chromium".to_owned()),
                });
        }
        state.library.active_account = Some(1);
        assert_eq!(
            press(&mut state, Key::A, egui::Modifiers::SHIFT),
            vec![Action::ToggleAudioOnly]
        );
    }
    #[test]
    fn arrow_up_and_down_request_vertical_page_scroll_while_j_and_k_move_the_cursor() {
        let videos = vec![
            crate::model::Video::from_id("aaaaaaaaaaa".into()),
            crate::model::Video::from_id("bbbbbbbbbbb".into()),
        ];
        let mut state = State {
            for_you: Listing::Ready(videos),
            ..State::default()
        };
        assert!(press(&mut state, Key::ArrowDown, egui::Modifiers::NONE).is_empty());
        assert_eq!(state.cursor, 0);
        assert!(!state.cursor_moved);
        assert_eq!(state.page_scroll, -KEY_SCROLL_STEP);

        state.page_scroll = 0.0;
        assert!(press(&mut state, Key::ArrowUp, egui::Modifiers::NONE).is_empty());
        assert_eq!(state.cursor, 0);
        assert!(!state.cursor_moved);
        assert_eq!(state.page_scroll, KEY_SCROLL_STEP);

        state.page_scroll = 0.0;
        assert!(press(&mut state, Key::J, egui::Modifiers::NONE).is_empty());
        assert_eq!(state.cursor, 1);
        assert!(state.cursor_moved);
        assert_eq!(state.page_scroll, 0.0);

        assert!(press(&mut state, Key::K, egui::Modifiers::NONE).is_empty());
        assert_eq!(state.cursor, 0);
        assert!(state.cursor_moved);
        assert_eq!(state.page_scroll, 0.0);
    }

    #[test]
    fn number_keys_follow_the_buttons_the_sidebar_shows() {
        use crate::storage::settings::{SidebarButton, SidebarEntry};
        let mut state = State::default();
        // Default order, signed out: Liked and Playlists are not drawn, so
        // 5 is Notifications.
        assert_eq!(
            press(&mut state, Key::Num5, egui::Modifiers::NONE),
            vec![Action::Navigate(View::Notifications)]
        );
        assert_eq!(
            press(&mut state, Key::Num7, egui::Modifiers::NONE),
            vec![Action::Navigate(View::Settings)]
        );
        assert!(press(&mut state, Key::Num8, egui::Modifiers::NONE).is_empty());
        // Hide Home and move Settings first.
        state.settings.sidebar = vec![
            SidebarEntry {
                button: SidebarButton::Settings,
                visible: true,
            },
            SidebarEntry {
                button: SidebarButton::Home,
                visible: false,
            },
        ];
        assert_eq!(
            press(&mut state, Key::Num1, egui::Modifiers::NONE),
            vec![Action::Navigate(View::Settings)]
        );
        assert_eq!(
            press(&mut state, Key::Num2, egui::Modifiers::NONE),
            vec![Action::Navigate(View::Subscriptions)]
        );
        // Signed in, Liked and Playlists count again.
        state.account = crate::app::AccountState::SignedIn(1);
        assert_eq!(
            press(&mut state, Key::Num5, egui::Modifiers::NONE),
            vec![Action::Navigate(View::Liked)]
        );
    }
}
