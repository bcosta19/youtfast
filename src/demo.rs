//! Deliberately isolated UI harness. No backend, keyring, user files, network,
//! media controls or single-instance channel are created here.
use crate::{
    app::{Page, State, apply_sidebar_action},
    appearance::i18n::{Locale, gettext as tr},
    appearance::theme,
    model::*,
    mpv::Status,
    ui,
};

pub fn run(cli: &crate::Cli) -> eframe::Result {
    let size = cli
        .demo_size
        .split_once('x')
        .and_then(|(w, h)| {
            Some([
                w.parse::<f32>().ok()?.clamp(480.0, 3840.0),
                h.parse::<f32>().ok()?.clamp(360.0, 2160.0),
            ])
        })
        .unwrap_or([1280.0, 800.0]);
    let page = cli.demo_view.clone();
    let locale = if cli.demo_language == "pt-BR" {
        Locale::PortugueseBrazil
    } else {
        Locale::English
    };
    let light = cli.demo_light;
    let capture = cli.demo_capture.clone();
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title(tr(locale, "YouTFast — demo").into_owned())
            .with_app_id("youtfast-demo")
            .with_inner_size(size),
        renderer: eframe::Renderer::Glow,
        persist_window: false,
        persistence_path: Some(
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("target/ux-validation/demo.ron"),
        ),
        ..Default::default()
    };
    eframe::run_native(
        "YouTFast Demo",
        options,
        Box::new(move |cc| {
            theme::install_fonts(&cc.egui_ctx);
            fastframe_icons::install::<crate::appearance::icons::Icon>(&cc.egui_ctx);
            egui_extras::install_image_loaders(&cc.egui_ctx);
            cc.egui_ctx
                .data_mut(|d| d.insert_temp(egui::Id::new("synthetic-images"), true));
            let mut state = fixture(&page);
            state.settings.language = locale;
            state.palette = if light {
                theme::Palette::light()
            } else {
                theme::Palette::dark()
            };
            theme::apply(&cc.egui_ctx, &state.palette);
            Ok(Box::new(Demo {
                state,
                status: Status {
                    position: 84.0,
                    duration: 600.0,
                    speed: 1.0,
                    paused: true,
                    volume: 65.0,
                    ..Default::default()
                },
                capture,
                frames: 0,
                hover_first: page == "home-hover",
            }))
        }),
    )
}

pub(crate) fn fixture(page: &str) -> State {
    let channel = Channel {
        id: "UCsynthetic".into(),
        name: "Demo channel".into(),
        avatar: None,
    };
    let videos: Vec<Video> = (0..18)
        .map(|i| Video {
            id: format!("demo{i:07}"),
            title: format!(
                "{} — a native experience for watching and discovering",
                [
                    "Travel around the world",
                    "Rust in practice",
                    "Music for concentration",
                    "Everyday science"
                ][i % 4]
            ),
            channel: Some(channel.name.clone()),
            channel_id: Some(channel.id.clone()),
            duration: Some(600.0 + i as f64 * 30.0),
            views: Some(12000 + i as u64 * 1300),
            published: Some("2 days ago".into()),
            ..Video::from_id(format!("demo{i:07}"))
        })
        .collect();
    let mut state = State {
        for_you: Listing::Ready(videos.clone()),
        feed: Listing::Ready(videos.clone()),
        feed_shown: 40,
        results: Listing::Ready(videos.clone()),
        search_query: "rust".into(),
        query: "rust".into(),
        up_next: videos.clone(),
        page_videos: Listing::Ready(videos.clone()),
        ..Default::default()
    };
    state.library.subscriptions.push(channel.clone());
    for v in videos.iter().take(8) {
        state.library.record_watch(v.clone(), crate::util::now());
    }
    state.library.watch_later = videos.iter().take(3).cloned().collect();
    state.channel_header = Some(ChannelHeader {
        id: channel.id.clone(),
        name: channel.name.clone(),
        handle: Some("@demo".into()),
        avatar: None,
        banner: Some("https://synthetic/banner".into()),
        subscribers: Some(120_000),
        videos: Some(48),
        has_shorts: true,
        has_live: true,
    });
    state.page = Some(Page::Channel(channel.clone()));
    state.watching = Some(videos[0].clone());
    state.queue = videos[1..4].to_vec();
    state.details=Fetch::Ready(Details{id:videos[0].id.clone(),title:videos[0].title.clone(),channel,subscribers:Some(120_000),views:12500,likes:Some(340),description:vec![Span::Text("A demo description with chapters, content and video information. All data in this window is synthetic.".into())],chapters:vec![Chapter{name:"Introduction".into(),seconds:0},Chapter{name:"Exploring the details".into(),seconds:60},Chapter{name:"Practical examples".into(),seconds:180}],related:videos[4..].to_vec(),..Default::default()});
    state.extras = Fetch::Ready(crate::youtube::extras::Extras {
        preview: None,
        language: "English".into(),
        transcript: Ok((0..50)
            .map(|i| crate::youtube::extras::Cue {
                seconds: i as f64 * 12.0,
                text: format!(
                    "Section {} of the demo transcript, synchronized with the video.",
                    i + 1
                ),
            })
            .collect()),
    });
    state.comments = Fetch::Ready(Vec::new());
    state.mini_closed = true;
    state.view = match page {
        "watch" | "transcript" | "shorts" | "fullscreen" | "cast" | "cast-connected"
        | "cast-missing" => View::Watch,
        "results" => View::Results,
        "channel" => View::Channel,
        "playlist" => View::Playlist,
        "history" => View::History,
        "settings" => View::Settings,
        "account" => View::Account,
        _ => View::Home,
    };
    if page == "account" {
        state
            .library
            .accounts
            .push(crate::storage::library::Account {
                id: 1,
                name: "Demo account".into(),
                source: AccountSource::File,
            });
    }
    if page == "playlist" {
        state.page = Some(Page::Playlist {
            id: "PLdemo".into(),
            name: "Demo playlist".into(),
        });
        state.playlist_details = Some(crate::youtube::innertube::PlaylistDetails {
            name: "Demo playlist".into(),
            thumbnail: Some("https://synthetic/playlist".into()),
            channel: Some("Demo channel".into()),
            description: "Content organized in a demo playlist.".into(),
            videos: Some(18),
        });
    }
    if page == "shorts" {
        state.watching.as_mut().unwrap().short = true;
        for v in &mut state.up_next {
            v.short = true;
        }
    }
    state.fullscreen = page == "fullscreen";
    state.keys_open = page == "help";
    state.transcript_open = page == "transcript";
    state.transcript_follow = true;
    if page == "loading" {
        state.for_you = Listing::Loading;
    }
    if page == "mini" {
        state.mini_closed = false;
    }
    if matches!(page, "cast" | "cast-connected" | "cast-missing") {
        use crate::desktop::cast::{Action, Device, Error, Reply};
        let device = Device {
            protocol: crate::desktop::cast::Protocol::GoogleCast,
            host: "192.168.1.20".parse().unwrap(),
            name: "Demo TV — Google Cast".into(),
        };
        state.cast.prepare(Action::Open, None, 0.0, 0);
        state.cast.finish(Reply::Devices(if page == "cast-missing" {
            Err(Error::MissingHelper)
        } else {
            Ok(vec![device.clone()])
        }));
        if page == "cast-connected" {
            state.cast.prepare(
                Action::Start(device.clone()),
                state.watching.as_ref(),
                84.0,
                0,
            );
            state.cast.finish(Reply::Started {
                device,
                video: state.watching.as_ref().unwrap().id.clone(),
                result: Ok(()),
            });
        }
    }
    state
}

struct Demo {
    state: State,
    status: Status,
    capture: Option<std::path::PathBuf>,
    frames: usize,
    hover_first: bool,
}
impl eframe::App for Demo {
    fn raw_input_hook(&mut self, _ctx: &egui::Context, input: &mut egui::RawInput) {
        if self.hover_first {
            // Synthetic input stays inside this demo viewport; the desktop's
            // actual pointer and the production instance are never moved.
            input
                .events
                .push(egui::Event::PointerMoved(egui::pos2(400.0, 220.0)));
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.frames += 1;
        let mut actions = Vec::new();
        ui::show(ui, &mut self.state, Some(&self.status), None, &mut actions);
        ui::release_speed_hold(ui.ctx(), &self.state, &mut actions);
        for action in actions {
            match action {
                Action::Cast(action) => {
                    // Keep the demo entirely offline: no helper, discovery or
                    // YouTube request is ever started from this harness.
                    use crate::desktop::cast::{Device, Reply, Request};
                    if let Some(request) = self.state.cast.prepare(
                        action,
                        self.state.watching.as_ref(),
                        self.status.position,
                        0,
                    ) {
                        let reply = match request {
                            Request::Disconnect => Reply::Disconnected,
                            Request::FindYouTube(host) => Reply::Devices(Ok(vec![Device {
                                host,
                                name: "Demo Samsung TV".into(),
                                protocol: crate::desktop::cast::Protocol::YouTube {
                                    endpoint: url::Url::parse(&format!(
                                        "http://{host}:8080/ws/apps/YouTube"
                                    ))
                                    .unwrap(),
                                },
                            }])),
                            Request::Scan => Reply::Devices(Ok(vec![Device {
                                protocol: crate::desktop::cast::Protocol::GoogleCast,
                                host: "192.168.1.20".parse().unwrap(),
                                name: "Demo TV — Google Cast".into(),
                            }])),
                            Request::Start { device, video, .. } => Reply::Started {
                                device,
                                video,
                                result: Ok(()),
                            },
                            Request::Control { control, .. } => Reply::Controlled {
                                control,
                                result: Ok(()),
                            },
                        };
                        if self.state.cast.finish(reply).is_some() {
                            self.status.paused = true;
                        }
                    }
                }
                Action::SetLanguage(locale) => {
                    self.state.settings.language = locale;
                    ui.ctx().send_viewport_cmd(egui::ViewportCommand::Title(
                        tr(locale, "YouTFast — demo").into_owned(),
                    ));
                }
                Action::Navigate(v) => self.state.view = v,
                Action::Open(v) => {
                    self.state.watching = Some(v);
                    self.state.view = View::Watch;
                    self.state.mini_closed = false;
                }
                Action::Back => {
                    self.state.view = View::Home;
                    self.state.mini_closed = false;
                }
                Action::ToggleMiniPlayer => {
                    self.state.view = if self.state.view == View::Watch {
                        View::Home
                    } else {
                        View::Watch
                    };
                    self.state.mini_closed = false;
                }
                Action::ReturnToWatch => self.state.view = View::Watch,
                Action::CloseMiniPlayer => self.state.mini_closed = true,
                Action::ToggleSidebar => {
                    apply_sidebar_action(
                        &mut self.state,
                        Action::ToggleSidebar,
                        ui.ctx().content_rect().width(),
                    );
                }
                Action::TogglePause => self.status.paused = !self.status.paused,
                Action::ToggleMute => self.status.muted = !self.status.muted,
                Action::SetSpeed(v) => self.status.speed = v,
                Action::SetVolume(v) => self.status.volume = v,
                Action::SeekTo { seconds, .. } => self.status.position = seconds,
                Action::SeekBy(v) => {
                    self.status.position =
                        (self.status.position + v).clamp(0.0, self.status.duration)
                }
                Action::SeekFraction(v) => self.status.position = v * self.status.duration,
                Action::ToggleTranscript => {
                    self.state.transcript_open = !self.state.transcript_open
                }
                Action::ToggleAudioOnly => {
                    self.state.audio_only = !self.state.audio_only;
                }
                Action::ToggleTheater => self.state.settings.theater = !self.state.settings.theater,
                Action::ToggleFullscreen => self.state.fullscreen = !self.state.fullscreen,
                Action::ExitFullscreen => self.state.fullscreen = false,
                Action::SetSearchOptions(v) => self.state.search_options = v,
                Action::SetChannelOrder(v) => self.state.channel_order = v,
                Action::SetHistoryQuery(v) => self.state.history_query = v,
                Action::RemoveHistory(id) => {
                    self.state.library.remove_history(&id);
                }
                Action::ToggleWatchLater(v) => {
                    self.state.library.toggle_watch_later(v);
                }
                Action::OpenChannel(c) => {
                    self.state.page = Some(Page::Channel(c));
                    self.state.view = View::Channel;
                }
                Action::SetChannelTab(v) => self.state.channel_tab = v,
                Action::SetChannelQuery(query) => self.state.channel_query = query,
                Action::MarkRenaming { id, name } => {
                    self.state.renaming = Some((id, name));
                    ui.ctx().memory_mut(|memory| {
                        memory.request_focus(egui::Id::new("rename account"));
                    });
                }
                Action::SetRenameDraft { id, name } => {
                    if let Some((editing, draft)) = &mut self.state.renaming
                        && *editing == id
                    {
                        *draft = name;
                    }
                }
                Action::RenameAccount { id, name } => {
                    if !name.trim().is_empty()
                        && let Some(account) = self.state.library.account_mut(id)
                    {
                        account.name = name.trim().to_owned();
                    }
                    self.state.renaming = None;
                }
                Action::Enqueue { video, next } => {
                    if next {
                        self.state.queue.insert(0, video);
                    } else {
                        self.state.queue.push(video);
                    }
                }
                Action::Dequeue(id) => self.state.queue.retain(|v| v.id != id),
                Action::MoveQueue { from, to } => {
                    if from < self.state.queue.len() && to < self.state.queue.len() {
                        let v = self.state.queue.remove(from);
                        self.state.queue.insert(to, v);
                    }
                }
                Action::ClearQueue => self.state.queue.clear(),
                Action::DismissNotice => self.state.notice = None,
                Action::CopyLink(_) => {
                    self.state.notice =
                        Some(tr(self.state.settings.language, "Link copied (demo)").into_owned())
                }
                _ => {}
            }
        }
        if self.frames >= 300 && self.capture.is_some() {
            ui.ctx()
                .send_viewport_cmd(egui::ViewportCommand::Screenshot(egui::UserData::default()));
        }
        let capture = ui.input(|i| {
            i.events.iter().find_map(|e| {
                if let egui::Event::Screenshot { image, .. } = e {
                    Some(image.clone())
                } else {
                    None
                }
            })
        });
        if let (Some(path), Some(image)) = (&self.capture, capture) {
            let bytes: Vec<u8> = image.pixels.iter().flat_map(|c| c.to_array()).collect();
            if let Some(parent) = path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            if image::save_buffer(
                path,
                &bytes,
                image.width() as u32,
                image.height() as u32,
                image::ColorType::Rgba8,
            )
            .is_err()
            {
                eprintln!("could not save demo capture");
            }
            ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
        }
        if self.capture.is_some() {
            ui.ctx().request_repaint();
        }
    }
}
