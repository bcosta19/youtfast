//! Deliberately isolated UI harness. No backend, keyring, user files, network,
//! media controls or single-instance channel are created here.
use youtfast_app::{Action, Page, State, apply_sidebar_action};
use youtfast_appearance::theme;
use youtfast_core::model::*;
use youtfast_i18n::{Locale, gettext as tr};
use youtfast_mpv::Status;
use youtfast_ui as ui;

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
    // The offline harness draws a synthetic picture where the video surface
    // would be; the opening and error views need it absent.
    let player_picture = !matches!(page.as_str(), "player-error" | "player-opening");
    let locale = if cli.demo_language == "pt-BR" {
        Locale::PortugueseBrazil
    } else {
        Locale::English
    };
    let light = cli.demo_light;
    let maximized = cli.demo_maximized;
    let aspect = match cli.demo_aspect.as_str() {
        "4:3" => 4.0 / 3.0,
        "21:9" => 21.0 / 9.0,
        "9:16" => 9.0 / 16.0,
        _ => 16.0 / 9.0,
    };
    let capture = cli.demo_capture.clone();
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title(tr(locale, "YouTFast — demo").into_owned())
            .with_app_id("youtfast-demo")
            .with_inner_size(size)
            .with_maximized(maximized),
        renderer: eframe::Renderer::Glow,
        persist_window: false,
        persistence_path: Some(
            std::path::PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../.."))
                .join("target/ux-validation/demo.ron"),
        ),
        ..Default::default()
    };
    eframe::run_native(
        "YouTFast Demo",
        options,
        Box::new(move |cc| {
            theme::install_fonts(&cc.egui_ctx);
            fastframe_icons::install::<youtfast_appearance::icons::Icon>(&cc.egui_ctx);
            egui_extras::install_image_loaders(&cc.egui_ctx);
            cc.egui_ctx.data_mut(|d| {
                d.insert_temp(egui::Id::new("synthetic-images"), true);
                d.insert_temp(egui::Id::new("synthetic-picture"), player_picture);
            });
            let mut state = fixture(&page);
            if page == "player-error" {
                state.player_error =
                    Some(youtfast_app::failure_message(youtfast_mpv::Failure::Other).into());
            }
            state.settings.language = locale;
            state.palette = if light {
                theme::Palette::light()
            } else {
                theme::Palette::dark()
            };
            theme::apply(&cc.egui_ctx, &state.palette);
            let mut status = Status {
                position: 84.0,
                duration: 600.0,
                speed: 1.0,
                paused: true,
                volume: 65.0,
                video_aspect: aspect,
                ..Default::default()
            };
            if page == "player-opening" {
                status.idle = true;
                status.buffering = true;
                status.position = 0.0;
                status.paused = false;
            }
            if page == "player-error" {
                status.idle = true;
                status.buffering = false;
                status.position = 0.0;
            }
            Ok(Box::new(Demo {
                state,
                status,
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
        state
            .library
            .record_watch(v.clone(), youtfast_core::util::now());
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
    state.extras = Fetch::Ready(youtfast_youtube::extras::Extras {
        preview: None,
        language: "English".into(),
        transcript: Ok((0..50)
            .map(|i| youtfast_youtube::extras::Cue {
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
        "watch" | "transcript" | "shorts" | "fullscreen" | "theater" | "cast"
        | "cast-connected" | "cast-missing" | "player-error" | "player-opening" => View::Watch,
        "results" => View::Results,
        "subscriptions" => View::Subscriptions,
        "channel" => View::Channel,
        "playlist" => View::Playlist,
        "history" => View::History,
        "watch-later" => View::WatchLater,
        "liked" => View::Liked,
        "playlists" => View::Playlists,
        "settings" => View::Settings,
        "account" => View::Account,
        _ => View::Home,
    };
    if page == "subscriptions" {
        populate_subscriptions_demo(&mut state);
    }
    if page == "home-shorts" {
        populate_home_shorts_demo(&mut state);
    }
    if matches!(page, "history" | "watch-later" | "liked" | "playlists") {
        populate_library_demo(&mut state, page);
    }
    if page == "notifications-popup" {
        state.alerts_visit = videos.iter().take(8).cloned().collect();
        state.notifications_popup_open = true;
    }
    if page == "account" {
        state
            .library
            .accounts
            .push(youtfast_storage::library::Account {
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
        state.playlist_details = Some(youtfast_youtube::innertube::PlaylistDetails {
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
    state.settings.theater = page == "theater";
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
        use youtfast_cast::{Action, Device, Error, Reply};
        let device = Device {
            protocol: youtfast_cast::Protocol::GoogleCast,
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
    if let Some(video) = state.watching.clone() {
        state.video_tabs.open(video);
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
                    use youtfast_cast::{Device, Reply, Request};
                    if let Some(request) = self.state.cast.prepare(
                        action,
                        self.state.watching.as_ref(),
                        self.status.position,
                        0,
                    ) {
                        let reply = match request {
                            Request::CheckSponsor { .. } => {
                                Reply::SponsorChecked(Ok(Default::default()))
                            }
                            Request::Disconnect => Reply::Disconnected,
                            Request::FindYouTube(host) => Reply::Devices(Ok(vec![Device {
                                host,
                                name: "Demo Samsung TV".into(),
                                protocol: youtfast_cast::Protocol::YouTube {
                                    endpoint: url::Url::parse(&format!(
                                        "http://{host}:8080/ws/apps/YouTube"
                                    ))
                                    .unwrap(),
                                },
                            }])),
                            Request::Link(_) => Reply::Devices(Ok(vec![Device {
                                protocol: youtfast_cast::Protocol::Linked {
                                    screen: "demo-screen".into(),
                                },
                                host: std::net::Ipv4Addr::UNSPECIFIED.into(),
                                name: "Demo TV — linked with a code".into(),
                            }])),
                            Request::Scan => Reply::Devices(Ok(vec![Device {
                                protocol: youtfast_cast::Protocol::GoogleCast,
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
                Action::VisitNotifications => {
                    if self.state.view != View::Notifications {
                        self.state.alerts_visit = std::mem::take(&mut self.state.alerts);
                    }
                }
                Action::MarkAlertsSeen => {
                    self.state.alerts.clear();
                    self.state.alerts_visit.clear();
                }
                Action::SetAlerts(on) => self.state.settings.alerts = on,
                Action::Navigate(v) => self.state.view = v,
                Action::Open(v) => {
                    self.state.video_tabs.open(v.clone());
                    self.state.watching = Some(v);
                    self.state.view = View::Watch;
                    self.state.mini_closed = false;
                }
                Action::OpenVideoTab(v) => self.state.video_tabs.add(v),
                Action::SelectVideoTab(id) => {
                    if let Some(video) = self.state.video_tabs.video(&id).cloned() {
                        self.state.video_tabs.open(video.clone());
                        self.state.watching = Some(video);
                        self.state.view = View::Watch;
                    }
                }
                Action::NewVideoTab => {
                    self.state.view = View::Home;
                    self.state.mini_closed = false;
                    self.state.video_tabs.request_new_tab();
                }
                Action::CloseVideoTab(id) => {
                    let active = self.state.video_tabs.active() == Some(id.as_str());
                    self.state.video_tabs.close(&id);
                    if active {
                        self.state.watching = self.state.video_tabs.selected().cloned();
                        if self.state.watching.is_none() {
                            self.state.view = View::Home;
                        }
                    }
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
                Action::SetWatchLaterQuery(v) => self.state.watch_later_query = v,
                Action::SetLikedQuery(v) => self.state.liked_query = v,
                Action::SetPlaylistsQuery(v) => self.state.playlists_query = v,
                Action::SetSubscriptionsQuery(v) => self.state.subscriptions_query = v,
                Action::SetSubscriptionsSort(v) => self.state.subscriptions_sort = v,
                Action::SetSource(v) => self.state.source = v,
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
        let capture_ready = if self.state.notifications_popup_open {
            self.frames >= 12
        } else {
            self.frames >= 300
        };
        if capture_ready && self.capture.is_some() {
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

fn populate_subscriptions_demo(state: &mut State) {
    let entries = [
        (
            "UCdemo_ayub",
            "Ayub",
            Some("@ayubdev"),
            Some(94_500),
            Some(182),
            Some("Software architecture, distributed systems and practical PostgreSQL tutorials."),
            true,
            Some((
                "subvid00001",
                "Zero-Downtime Migrations in Practice",
                42_000,
                "5 hours ago",
                false,
            )),
            3,
        ),
        (
            "UCdemo_clojure",
            "ClojureTV",
            Some("@ClojureTV"),
            Some(68_200),
            Some(615),
            Some(
                "Recorded talks from Clojure/conj, EuroClojure and community functional programming meetups.",
            ),
            true,
            Some((
                "subvid00002",
                "Building Reliable Concurrent Pipelines in Clojure",
                19_400,
                "1 day ago",
                false,
            )),
            2,
        ),
        (
            "UCdemo_comp",
            "Computerphile",
            Some("@Computerphile"),
            Some(2_640_000),
            Some(850),
            Some(
                "Videos all about computers and computer stuff, hosted by researchers and engineers.",
            ),
            true,
            Some((
                "subvid00003",
                "Why Branch Prediction Changed Everything",
                310_000,
                "6 hours ago",
                false,
            )),
            5,
        ),
        (
            "UCdemo_akita",
            "Fabio Akita",
            Some("@Akitando"),
            Some(512_000),
            Some(240),
            Some(
                "Deep technical dives into operating systems, Linux, networking and software craft.",
            ),
            true,
            Some((
                "subvid00004",
                "How Linux Memory Allocation Actually Works",
                145_000,
                "2 days ago",
                false,
            )),
            4,
        ),
        (
            "UCdemo_ken",
            "Ken",
            Some("@ken_builds"),
            Some(41_000),
            Some(96),
            Some("Hands-on systems builds, hardware teardowns and terminal tooling."),
            false,
            None,
            1,
        ),
        (
            "UCdemo_laravel",
            "Laravel Daily",
            Some("@LaravelDaily"),
            Some(175_000),
            Some(980),
            Some("Daily backend engineering tips, query optimization and test-driven workflows."),
            true,
            Some((
                "subvid00005",
                "Debugging N+1 Queries Without Guesswork",
                28_000,
                "Live",
                true,
            )),
            0,
        ),
        (
            "UCdemo_rails",
            "Ruby on Rails",
            Some("@rubyonrails"),
            Some(89_000),
            Some(410),
            Some("Official framework releases, Rails World keynotes and core engineering updates."),
            true,
            Some((
                "subvid00006",
                "Rails World 2026: Native Speed & Simplicity",
                64_000,
                "4 days ago",
                false,
            )),
            0,
        ),
        (
            "UCdemo_pg",
            "Smart Postgres",
            Some("@smartpostgres"),
            Some(32_800),
            Some(74),
            Some(
                "Query plans, vacuum tuning, B-tree internals and indexing strategies for high throughput.",
            ),
            false,
            None,
            0,
        ),
        (
            "UCdemo_prime",
            "The PrimeTime",
            Some("@ThePrimeTimeagen"),
            Some(740_000),
            Some(1_420),
            Some(
                "Systems performance, code reviews, compiler discussions and programming streams.",
            ),
            true,
            Some((
                "subvid00007",
                "Reading Open Source Codebases Like a Pro",
                215_000,
                "3 hours ago",
                false,
            )),
            6,
        ),
    ];

    let mut channels = Vec::new();
    let mut feed_videos = Vec::new();
    state.library.history.clear();
    let now = youtfast_core::util::now();

    for (idx, (id, name, handle, subs, vids, bio, verified, latest, watched_n)) in
        entries.into_iter().enumerate()
    {
        channels.push(Channel {
            id: id.into(),
            name: name.into(),
            avatar: Some(format!("https://synthetic/avatar-{id}")),
        });
        state.merge_channel_meta(
            id.into(),
            ChannelMeta {
                handle: handle.map(str::to_owned),
                subscribers: subs,
                videos: vids,
                description: bio.map(str::to_owned),
                verified,
            },
        );
        if let Some((vid_id, title, views, published, live)) = latest {
            feed_videos.push(Video {
                id: vid_id.into(),
                title: title.into(),
                channel: Some(name.into()),
                channel_id: Some(id.into()),
                channel_avatar: Some(format!("https://synthetic/avatar-{id}")),
                duration: (!live).then_some(780.0 + idx as f64 * 95.0),
                views: Some(views),
                live,
                published_at: (!live).then_some(now - (idx as i64 + 1) * 7_200),
                published: Some(published.into()),
                short: false,
            });
        }
        for w in 0..watched_n {
            state.library.record_watch(
                Video {
                    title: format!("{name} — watched session #{}", w + 1),
                    channel: Some(name.into()),
                    channel_id: Some(id.into()),
                    duration: Some(920.0),
                    views: Some(15_000),
                    published: Some("1 week ago".into()),
                    ..Video::from_id(format!("hist{idx:02}_{w:04}"))
                },
                now - (w as i64 * 3_600),
            );
        }
    }

    state.account = youtfast_app::AccountState::SignedIn(1);
    state.library.active_account = Some(1);
    if state.library.accounts.is_empty() {
        state
            .library
            .accounts
            .push(youtfast_storage::library::Account {
                id: 1,
                name: "Work".into(),
                source: AccountSource::File,
            });
    }
    state.source = Source::Account;
    state.query.clear();
    state.watching = None;
    state.video_tabs = Default::default();
    state.library.subscriptions = channels.iter().take(4).cloned().collect();
    state.account_subscriptions = Listing::Ready(channels);
    state.feed = Listing::Ready(feed_videos);
    state.alerts.push(state.feed.items()[0].clone());
}

fn populate_home_shorts_demo(state: &mut State) {
    populate_subscriptions_demo(state);
    state.view = View::Home;
    state.feed_filter = FeedFilter::All;
    let now = youtfast_core::util::now();
    if let Listing::Ready(feed) = &mut state.feed {
        for (idx, video) in feed.iter_mut().enumerate() {
            let offset = if idx < 3 {
                (idx as i64 + 1) * 1_800
            } else {
                24 * 3_600 + idx as i64 * 7_200
            };
            video.published_at = Some(now - offset);
        }
    }
    let shorts = [
        (
            "shrtdemo01",
            "Qual é o aprendizado do apagão do Microsoft 365?",
            Some("Fabio Akita"),
            Some("UCakita"),
            144,
        ),
        (
            "shrtdemo02",
            "How to help AI produce great PR reviews",
            Some("Laravel Daily"),
            Some("UClaravel"),
            1_100,
        ),
        (
            "shrtdemo03",
            "Hero Magic Archer is completely broken",
            Some("Ken"),
            Some("UCken"),
            411_000,
        ),
        (
            "shrtdemo04",
            "thanks supercell for this wild balance patch",
            Some("Ken"),
            Some("UCken"),
            853_000,
        ),
        (
            "shrtdemo05",
            "Evolved Mega Knight vs Pekka timing guide",
            None,
            None,
            867_000,
        ),
        (
            "shrtdemo06",
            "GOBLIN QUEEN crazy comeback in overtime",
            Some("Ken"),
            Some("UCken"),
            451_000,
        ),
        (
            "shrtdemo07",
            "Goblin Queen clutch tower defense",
            None,
            None,
            1_200_000,
        ),
        (
            "shrtdemo08",
            "this is so cool in modern Rust UI",
            Some("The PrimeTime"),
            Some("UCprime"),
            254_000,
        ),
        (
            "shrtdemo09",
            "Why indexes speed up PostgreSQL queries",
            Some("Smart Postgres"),
            Some("UCpg"),
            92_000,
        ),
        (
            "shrtdemo10",
            "Ruby on Rails 8 Solid Queue in 45 seconds",
            Some("Ruby on Rails"),
            Some("UCrails"),
            64_000,
        ),
    ];
    state.shorts = Listing::Ready(
        shorts
            .into_iter()
            .map(|(id, title, channel, channel_id, views)| Video {
                title: title.into(),
                channel: channel.map(str::to_owned),
                channel_id: channel_id.map(str::to_owned),
                views: Some(views),
                short: true,
                ..Video::from_id(id.into())
            })
            .collect(),
    );
}

fn populate_library_demo(state: &mut State, page: &str) {
    populate_subscriptions_demo(state);
    state.view = match page {
        "watch-later" => View::WatchLater,
        "liked" => View::Liked,
        "playlists" => View::Playlists,
        _ => View::History,
    };
    let now = youtfast_core::util::now();
    let sample_videos = [
        (
            "libvid00001",
            "Zero-Downtime PostgreSQL Migrations at Scale",
            "Ayub",
            "UCayub",
            1_420.0,
            42_000,
            "5 hours ago",
            615.0,
        ),
        (
            "libvid00002",
            "Building Reliable Concurrent Pipelines in Clojure",
            "ClojureTV",
            "UCclojure",
            1_845.0,
            19_000,
            "1 day ago",
            0.0,
        ),
        (
            "libvid00003",
            "Why Branch Prediction Changed Modern CPUs",
            "Computerphile",
            "UCcomp",
            1_090.0,
            310_000,
            "6 hours ago",
            480.0,
        ),
        (
            "libvid00004",
            "How Linux Memory Allocation Actually Works",
            "Fabio Akita",
            "UCakita",
            2_760.0,
            145_000,
            "2 days ago",
            1_250.0,
        ),
        (
            "libvid00005",
            "Laravel 12 Queues and Worker Isolation Deep Dive",
            "Laravel Daily",
            "UClaravel",
            980.0,
            31_000,
            "1 day ago",
            0.0,
        ),
        (
            "libvid00006",
            "PostgreSQL 17 Vacuum, BRIN & Partial Indexes Explained",
            "Smart Postgres",
            "UCpg",
            1_340.0,
            28_500,
            "3 days ago",
            0.0,
        ),
        (
            "libvid00007",
            "Reading Open Source Compilers Without Getting Lost",
            "The PrimeTime",
            "UCprime",
            1_620.0,
            215_000,
            "3 hours ago",
            320.0,
        ),
        (
            "libvid00008",
            "Rails 8 Deployment From Scratch With Kamal",
            "Ruby on Rails",
            "UCrails",
            1_150.0,
            64_000,
            "4 days ago",
            0.0,
        ),
    ];

    let built_videos: Vec<Video> = sample_videos
        .iter()
        .map(
            |(id, title, channel, channel_id, duration, views, published, _)| Video {
                title: (*title).into(),
                channel: Some((*channel).into()),
                channel_id: Some((*channel_id).into()),
                channel_avatar: Some(format!("https://synthetic/avatar-{channel_id}")),
                duration: Some(*duration),
                views: Some(*views),
                published: Some((*published).into()),
                ..Video::from_id((*id).into())
            },
        )
        .collect();

    state.library.watch_later = built_videos.clone();
    state.account_watch_later = Listing::Ready(built_videos.clone());
    state.liked = Listing::Ready(built_videos.clone());

    state.settings.history_sync = youtfast_storage::settings::HistorySync::Merged;
    state.library.history.clear();
    for (idx, ((_, _, _, _, _, _, _, pos), video)) in
        sample_videos.iter().zip(built_videos.iter()).enumerate()
    {
        state
            .library
            .history
            .push(youtfast_storage::library::Watched {
                video: video.clone(),
                at: now - (idx as i64 * 7_200 + 300),
                position: *pos,
            });
    }
    state.account_history_generation += 1;
    state.account_history = Listing::Ready(vec![
        HistoryRow {
            video: built_videos[0].clone(),
            when: Some("Today".into()),
        },
        HistoryRow {
            video: built_videos[2].clone(),
            when: Some("Today".into()),
        },
        HistoryRow {
            video: built_videos[3].clone(),
            when: Some("Yesterday".into()),
        },
        HistoryRow {
            video: built_videos[5].clone(),
            when: Some("Yesterday".into()),
        },
    ]);

    state.playlists = Listing::Ready(vec![
        PlaylistSummary {
            id: "PL_systems".into(),
            name: "Systems Engineering & Operating Systems".into(),
            thumbnail: Some("https://synthetic/pl-systems".into()),
            channel: Some("Fabio Akita".into()),
            videos: Some(24),
        },
        PlaylistSummary {
            id: "PL_postgres".into(),
            name: "Production PostgreSQL Internals".into(),
            thumbnail: Some("https://synthetic/pl-postgres".into()),
            channel: Some("Smart Postgres".into()),
            videos: Some(18),
        },
        PlaylistSummary {
            id: "PL_architecture".into(),
            name: "Distributed Systems & Migrations".into(),
            thumbnail: Some("https://synthetic/pl-arch".into()),
            channel: Some("Ayub".into()),
            videos: Some(12),
        },
        PlaylistSummary {
            id: "PL_clojure".into(),
            name: "Functional Concurrency Talks".into(),
            thumbnail: Some("https://synthetic/pl-clojure".into()),
            channel: Some("ClojureTV".into()),
            videos: Some(31),
        },
        PlaylistSummary {
            id: "PL_comp".into(),
            name: "Hardware Architecture Classics".into(),
            thumbnail: Some("https://synthetic/pl-comp".into()),
            channel: Some("Computerphile".into()),
            videos: Some(46),
        },
        PlaylistSummary {
            id: "PL_empty".into(),
            name: "Saved For Weekend Reading".into(),
            thumbnail: None,
            channel: Some("Work".into()),
            videos: Some(5),
        },
    ]);
}
