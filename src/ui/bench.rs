//! What one frame costs on the busiest views.
//!
//! Run with:
//! `cargo test --release --bin youtfast frame_cost -- --ignored --nocapture --test-threads=1`
//!
//! Each scenario draws the whole window (`ui::show`) from synthetic data: no
//! network, no player. The thumbnail cache is never installed in tests, so
//! images stay pending and nothing is requested. Only CPU time spent on the
//! interface thread is measured; nothing is rasterised or uploaded.

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

use egui::vec2;

use super::show;
use crate::app::{AccountState, State};
use crate::logic::history::HistoryFilter;
use crate::model::{
    Channel, CommentRow, Details, FeedFilter, Fetch, HistoryRow, Listing, Span, Video, View,
};
use crate::storage::library::Watched;
use crate::storage::settings::HistorySync;

/// Counts allocations so a run can report them next to the time.
struct Counting;

static ALLOCATIONS: AtomicU64 = AtomicU64::new(0);

// SAFETY: every call is forwarded unchanged to the system allocator.
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        ALLOCATIONS.fetch_add(1, Ordering::Relaxed);
        unsafe { System.alloc(layout) }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) }
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        ALLOCATIONS.fetch_add(1, Ordering::Relaxed);
        unsafe { System.realloc(ptr, layout, new_size) }
    }
}

#[global_allocator]
static ALLOCATOR: Counting = Counting;

const WARM_UP: usize = 30;
const FRAMES: usize = 300;
const HOUR: i64 = 3600;

fn video(index: usize, published_at: Option<i64>) -> Video {
    let mut video = Video::from_id(format!("{index:011}"));
    video.title = format!(
        "Video number {index}{} with a title long enough to wrap onto a second line",
        if index % 10 == 0 { " alpha" } else { "" }
    );
    video.channel = Some(format!("Channel {}", index % 37));
    video.channel_id = Some(format!("UC{:022}", index % 37));
    video.duration = Some(60.0 + (index % 90) as f64 * 40.0);
    video.views = Some(1_000 + index as u64 * 733);
    video.published_at = published_at;
    video.published = Some(format!("{} days ago", index % 30));
    video
}

/// `count` videos, newest first, `step` seconds apart, from now.
fn videos(count: usize, step: i64) -> Vec<Video> {
    let now = crate::util::now();
    (0..count)
        .map(|index| video(index, Some(now - index as i64 * step)))
        .collect()
}

fn state(view: View) -> State {
    let mut state = State {
        view,
        ..Default::default()
    };
    state.settings.alerts = true;
    state
}

fn history(query: &str) -> State {
    let now = crate::util::now();
    let mut state = state(View::History);
    state.account = AccountState::SignedIn(1);
    state.settings.history_sync = HistorySync::Merged;
    state.history_filter = HistoryFilter::All;
    state.history_query = query.to_owned();
    state.library.history = (0..1000)
        .map(|index| Watched {
            video: video(index, None),
            at: now - index as i64 * HOUR * 2,
            position: (index % 50) as f64 * 10.0,
        })
        .collect();
    state.account_history = Listing::Ready(
        (1000..1200)
            .map(|index| HistoryRow {
                video: video(index, None),
                when: Some(
                    match index % 100 {
                        0..=39 => "Today",
                        40..=79 => "Yesterday",
                        _ => "Monday",
                    }
                    .to_owned(),
                ),
            })
            .collect(),
    );
    state
}

fn home() -> State {
    let mut state = state(View::Home);
    state.feed = Listing::Ready(videos(400, 3 * HOUR));
    state.feed_shown = 400;
    state.feed_filter = FeedFilter::All;
    // An empty library would show the "Your feed is empty" welcome instead.
    state.library.subscriptions = vec![Channel {
        id: "UC0000000000000000000001".to_owned(),
        name: "Channel 1".to_owned(),
        avatar: None,
    }];
    state
}

fn notifications() -> State {
    let mut state = state(View::Notifications);
    state.alerts_visit = videos(300, HOUR);
    state
}

fn spans(count: usize) -> Vec<Span> {
    let mut spans = Vec::new();
    for index in 0..count {
        spans.push(Span::Text(format!("Line {index} of the description, ")));
        spans.push(if index % 2 == 0 {
            Span::Link {
                text: format!("https://example.com/{index}"),
                url: format!("https://example.com/{index}"),
            }
        } else {
            Span::Seek {
                text: format!("{index}:{:02}", index % 60),
                seconds: index as u32 * 7,
            }
        });
        spans.push(Span::Text("\n".to_owned()));
    }
    spans
}

fn watch() -> State {
    let mut state = state(View::Watch);
    let watching = video(1, Some(crate::util::now() - 5 * HOUR));
    let channel = Channel {
        id: "UC0000000000000000000001".to_owned(),
        name: "Channel 1".to_owned(),
        avatar: None,
    };
    state.details = Fetch::Ready(Details {
        id: watching.id.clone(),
        title: watching.title.clone(),
        channel,
        subscribers: Some(120_000),
        views: 48_000,
        likes: Some(2_300),
        published: Some("Oct 4, 2026".to_owned()),
        published_date: Some("2026-10-04".to_owned()),
        live: false,
        description: spans(40),
        chapters: Vec::new(),
        related: (0..20).map(|index| video(2_000 + index, None)).collect(),
        comments_token: None,
        comment_count: Some(200),
        visitor_data: None,
    });
    state.comments = Fetch::Ready(
        (0..200)
            .map(|index| CommentRow {
                id: format!("comment-{index}"),
                author: Some(format!("@viewer{index}")),
                author_id: Some(format!("UC{index:022}")),
                author_avatar: Some(format!("https://yt3.example/avatar/{index}")),
                text: spans(1 + index % 3),
                image: None,
                published: format!("{} days ago", index % 30),
                likes: Some(index as u32),
                replies: (index % 4) as u32,
                replies_token: None,
                pinned: index == 0,
                by_owner: index % 25 == 0,
                hearted: index % 10 == 0,
            })
            .collect(),
    );
    state.watching = Some(watching);
    state
}

fn results() -> State {
    let mut state = state(View::Results);
    state.search_query = "alpha".to_owned();
    state.results = Listing::Ready(videos(200, 6 * HOUR));
    state
}

/// Draws one frame; every tenth frame the pointer moves, so hover work is
/// part of the mix without making the run depend on timing.
fn frame(ctx: &egui::Context, state: &mut State, index: usize) {
    let events = if index % 10 == 0 {
        let x = 300.0 + (index % 50) as f32 * 12.0;
        vec![egui::Event::PointerMoved(egui::pos2(x, 400.0))]
    } else {
        Vec::new()
    };
    let mut actions = Vec::new();
    let mut output = ctx.run_ui(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                vec2(1512.0, 900.0),
            )),
            time: Some(index as f64 / 60.0),
            predicted_dt: 1.0 / 60.0,
            events,
            ..Default::default()
        },
        |ui| show(ui, state, None, None, &mut actions),
    );
    output.textures_delta.clear();
    std::hint::black_box((&output.shapes, &actions));
}

fn measure(name: &str, mut state: State) {
    let ctx = egui::Context::default();
    crate::appearance::theme::install_fonts(&ctx);
    for index in 0..WARM_UP {
        frame(&ctx, &mut state, index);
    }
    let mut times = Vec::with_capacity(FRAMES);
    let allocations = ALLOCATIONS.load(Ordering::Relaxed);
    for index in WARM_UP..WARM_UP + FRAMES {
        let start = Instant::now();
        frame(&ctx, &mut state, index);
        times.push(start.elapsed().as_secs_f64() * 1000.0);
    }
    let allocations = (ALLOCATIONS.load(Ordering::Relaxed) - allocations) / FRAMES as u64;
    let mean = times.iter().sum::<f64>() / FRAMES as f64;
    times.sort_by(f64::total_cmp);
    let (p50, p95) = (times[FRAMES / 2], times[FRAMES * 95 / 100]);
    println!(
        "frame_cost {name} mean={mean:.3}ms p50={p50:.3}ms p95={p95:.3}ms frames={FRAMES} allocs_per_frame={allocations}"
    );
}

#[test]
#[ignore = "a measurement, not a check; see the module docs"]
fn frame_cost() {
    measure("history", history(""));
    measure("history_search", history("alpha"));
    measure("home", home());
    measure("notifications", notifications());
    measure("watch", watch());
    measure("results", results());
}
