//! Recommendations made on this computer, for Home's For you when the
//! account has none to give (signed out, or its history paused): the
//! videos YouTube lists as related to what was watched lately, ranked.
//!
//! A video watched recently and to its end counts more as a seed; a
//! related video counts more the higher YouTube placed it, the more seeds
//! list it, and when its channel is one the user follows or watches
//! often. What was already watched is left out, and no channel takes more
//! than its share of the top.

use std::collections::{HashMap, HashSet};

use youtfast_core::model::{Channel, Video};
use youtfast_storage::library::{Library, Watched};
use youtfast_storage::settings::{
    RecommendationChannels, RecommendationDuration, RecommendationPriority, RecommendationTopic,
    Recommendations, recommendation_terms,
};

/// How many watched videos seed the list: each costs a details request.
pub const SEEDS: usize = 12;
/// At most this many of one channel in a row of the ranking before the
/// others get their turn.
const PER_CHANNEL: usize = 2;
/// Number of eligible watches considered for channel affinity.
const AFFINITY_WATCHES: usize = 200;
/// How long the list is.
const KEPT: usize = 80;

/// A video watched lately and how much it says about the user's taste.
#[derive(Clone, Debug, PartialEq)]
pub struct Seed {
    pub id: String,
    pub weight: f64,
}

/// What the ranking knows about the user besides the seeds.
#[derive(Clone, Debug, Default)]
pub struct Taste {
    /// Ids not to recommend: the whole history.
    pub watched: HashSet<String>,
    /// How much each channel is liked (subscribed, watched often), 0 to 1.
    pub channels: HashMap<String, f64>,
    /// Hidden video ids not to recommend.
    pub hidden_videos: HashSet<String>,
    /// Blocked channel ids not to recommend.
    pub blocked_channels: HashSet<String>,
}

/// The seeds: the latest distinct videos of the history, not Shorts or
/// live streams (their related lists are about the moment), weighed by
/// how recent they are and how much of them was watched.
pub fn seeds(history: &[Watched]) -> Vec<Seed> {
    seeds_filtered(history, |_| false)
}

/// Like [`seeds`], skipping videos hidden or from channels blocked in `library`.
pub fn seeds_for_library(library: &Library) -> Vec<Seed> {
    seeds_filtered(&library.history, |video| library.is_hidden(video))
}

/// Whether the device history has any video that can seed recommendations.
pub fn has_eligible_history(library: &Library) -> bool {
    eligible_watches(&library.history, |video| library.is_hidden(video))
        .next()
        .is_some()
}

fn eligible_watches(
    history: &[Watched],
    is_hidden: impl Fn(&Video) -> bool,
) -> impl Iterator<Item = &Watched> {
    history.iter().filter(move |watched| {
        !watched.video.short && !watched.video.live && !is_hidden(&watched.video)
    })
}

fn seeds_filtered(history: &[Watched], is_hidden: impl Fn(&Video) -> bool) -> Vec<Seed> {
    let mut seen = HashSet::new();
    eligible_watches(history, is_hidden)
        .filter(|watched| seen.insert(watched.video.id.clone()))
        .take(SEEDS)
        .enumerate()
        .map(|(rank, watched)| {
            let recency = 1.0 / (1.0 + rank as f64 * 0.25);
            // Zero is finished (or never started); a part left is how far
            // it got.
            let watched_part = match watched.video.duration {
                Some(duration) if duration > 0.0 && watched.position > 0.0 => {
                    (watched.position / duration).clamp(0.2, 1.0)
                }
                _ => 0.8,
            };
            Seed {
                id: watched.video.id.clone(),
                weight: recency * watched_part,
            }
        })
        .collect()
}

/// The channels the user cares for: subscribed ones fully, the others by
/// how often the history has them.
pub fn taste(history: &[Watched], subscriptions: &[Channel]) -> Taste {
    taste_filtered(history, subscriptions, |_| false, |_| false)
}

fn taste_filtered(
    history: &[Watched],
    subscriptions: &[Channel],
    excluded_watch: impl Fn(&Video) -> bool,
    blocked_channel: impl Fn(&str) -> bool,
) -> Taste {
    let mut counts: HashMap<String, f64> = HashMap::new();
    let recent = history
        .iter()
        .filter(|watched| !excluded_watch(&watched.video))
        .take(AFFINITY_WATCHES);
    for watched in recent {
        if let Some(channel) = &watched.video.channel_id {
            *counts.entry(channel.clone()).or_default() += 1.0;
        }
    }
    let most = counts.values().copied().fold(1.0, f64::max);
    for count in counts.values_mut() {
        *count /= most;
    }
    for channel in subscriptions {
        if !blocked_channel(&channel.id) {
            counts.insert(channel.id.clone(), 1.0);
        }
    }
    Taste {
        watched: history
            .iter()
            .map(|watched| watched.video.id.clone())
            .collect(),
        channels: counts,
        hidden_videos: HashSet::new(),
        blocked_channels: HashSet::new(),
    }
}

/// Like [`taste`], populated with `library`'s hidden videos and blocked channels.
pub fn taste_for_library(library: &Library) -> Taste {
    let mut t = taste_filtered(
        &library.history,
        &library.subscriptions,
        |video| library.is_hidden(video),
        |id| library.is_channel_blocked(id),
    );
    t.hidden_videos = library.hidden_videos().iter().cloned().collect();
    t.blocked_channels = library
        .blocked_channels()
        .iter()
        .map(|channel| channel.id.clone())
        .collect();
    t
}

/// The ranked list from each seed's weight and its related videos, in
/// YouTube's order.
pub fn rank(related: &[(f64, Vec<Video>)], taste: &Taste) -> Vec<Video> {
    rank_with(related, taste, &Recommendations::default(), 0)
}

pub fn rank_with(
    related: &[(f64, Vec<Video>)],
    taste: &Taste,
    preferences: &Recommendations,
    now: i64,
) -> Vec<Video> {
    rank_combined(related, &[], taste, preferences, now)
}

/// Ranks history-based related videos together with explicit topic search
/// buckets, interleaving active topics with history so one watched niche does
/// not crowd out chosen topics.
pub fn rank_combined(
    history: &[(f64, Vec<Video>)],
    topics: &[Vec<Video>],
    taste: &Taste,
    preferences: &Recommendations,
    now: i64,
) -> Vec<Video> {
    let muted: Vec<_> = recommendation_terms(&preferences.muted)
        .map(str::to_lowercase)
        .collect();
    let interests: Vec<_> = recommendation_terms(&preferences.interests)
        .map(str::to_lowercase)
        .collect();

    let history_ranked = rank_history(history, taste, preferences, &muted, &interests, now);
    let (topic_ranked, active_topics) =
        rank_topics(topics, taste, preferences, &muted, &interests, now);

    if topic_ranked.is_empty() {
        return history_ranked.into_iter().take(KEPT).collect();
    }
    if history_ranked.is_empty() {
        return finish_lane(topic_ranked, preferences.channels);
    }

    let topics_per_history = if active_topics >= 2 { 2 } else { 1 };
    let merged = interleave_lanes(
        topic_ranked,
        history_ranked,
        topics_per_history,
        preferences.channels,
    );
    if preferences.channels == RecommendationChannels::Discover {
        merged
    } else {
        spread(merged)
    }
}

fn rank_history(
    related: &[(f64, Vec<Video>)],
    taste: &Taste,
    preferences: &Recommendations,
    muted: &[String],
    interests: &[String],
    now: i64,
) -> Vec<Video> {
    // Track best single-seed score separately from extra-seed agreement so one
    // repeatedly watched channel cannot overpower every filter preference.
    let mut scores: HashMap<String, (f64, f64, Video)> = HashMap::new();
    for (weight, videos) in related {
        for (place, video) in videos.iter().enumerate() {
            if !is_candidate(video, taste, preferences.duration, muted) {
                continue;
            }
            let score = weight / (1.0 + place as f64 * 0.15);
            scores
                .entry(video.id.clone())
                .and_modify(|(best, extra, _)| {
                    if score > *best {
                        *extra += *best;
                        *best = score;
                    } else {
                        *extra += score;
                    }
                })
                .or_insert((score, 0.0, video.clone()));
        }
    }
    let mut ranked: Vec<(f64, Video)> = scores
        .into_values()
        .map(|(best, extra, video)| {
            let base = best + 0.35 * extra;
            let mult = channel_multiplier(&video, taste, preferences.channels)
                * topic_multiplier(&video, &preferences.topics, interests)
                * priority_multiplier(&video, preferences.priority, now);
            (base * mult, video)
        })
        .collect();
    ranked.sort_by(|a, b| b.0.total_cmp(&a.0).then_with(|| a.1.id.cmp(&b.1.id)));
    finish_lane(
        ranked.into_iter().map(|(_, video)| video).collect(),
        preferences.channels,
    )
}

fn rank_topics(
    topics: &[Vec<Video>],
    taste: &Taste,
    preferences: &Recommendations,
    muted: &[String],
    interests: &[String],
    now: i64,
) -> (Vec<Video>, usize) {
    let mut lanes: Vec<std::collections::VecDeque<Video>> = topics
        .iter()
        .map(|videos| {
            let mut seen = HashSet::new();
            let mut ranked: Vec<(f64, Video)> = videos
                .iter()
                .enumerate()
                .filter(|(_, video)| {
                    is_candidate(video, taste, preferences.duration, muted)
                        && seen.insert(video.id.clone())
                })
                .map(|(place, video)| {
                    let base = 1.0 / (1.0 + place as f64 * 0.15);
                    let mult = channel_multiplier(video, taste, preferences.channels)
                        * custom_interest_multiplier(video, interests)
                        * priority_multiplier(video, preferences.priority, now);
                    (base * mult, video.clone())
                })
                .collect();
            ranked.sort_by(|a, b| b.0.total_cmp(&a.0).then_with(|| a.1.id.cmp(&b.1.id)));
            finish_lane(
                ranked.into_iter().map(|(_, video)| video).collect(),
                preferences.channels,
            )
            .into()
        })
        .collect();
    let active_topics = lanes.iter().filter(|lane| !lane.is_empty()).count();

    let mut out = Vec::new();
    let mut seen_ids = HashSet::new();
    loop {
        let mut progressed = false;
        for lane in &mut lanes {
            while let Some(video) = lane.pop_front() {
                if seen_ids.insert(video.id.clone()) {
                    out.push(video);
                    progressed = true;
                    break;
                }
            }
        }
        if !progressed {
            break;
        }
    }
    (out, active_topics)
}

fn finish_lane(mut ranked: Vec<Video>, channels: RecommendationChannels) -> Vec<Video> {
    if channels == RecommendationChannels::Discover {
        let mut seen_channels = HashSet::new();
        ranked.retain(|video| {
            video
                .channel_id
                .as_ref()
                .is_none_or(|id| seen_channels.insert(id.clone()))
        });
        return ranked.into_iter().take(KEPT).collect();
    }
    spread(ranked)
}

fn interleave_lanes(
    topics: Vec<Video>,
    history: Vec<Video>,
    topics_per_history: usize,
    channels: RecommendationChannels,
) -> Vec<Video> {
    let limit = if channels == RecommendationChannels::Discover {
        KEPT
    } else {
        topics.len() + history.len()
    };
    let mut topic_iter = topics.into_iter();
    let mut history_iter = history.into_iter();
    let mut seen_ids = HashSet::new();
    let mut seen_channels = HashSet::new();
    let mut out = Vec::with_capacity(limit.min(KEPT));

    let mut next_video = |iter: &mut std::vec::IntoIter<Video>| -> Option<Video> {
        for video in iter.by_ref() {
            if !seen_ids.insert(video.id.clone()) {
                continue;
            }
            if channels == RecommendationChannels::Discover
                && let Some(channel_id) = &video.channel_id
                && !seen_channels.insert(channel_id.clone())
            {
                continue;
            }
            return Some(video);
        }
        None
    };

    while out.len() < limit {
        let mut progressed = false;
        for _ in 0..topics_per_history {
            if out.len() >= limit {
                break;
            }
            if let Some(video) = next_video(&mut topic_iter) {
                out.push(video);
                progressed = true;
            }
        }
        if out.len() < limit
            && let Some(video) = next_video(&mut history_iter)
        {
            out.push(video);
            progressed = true;
        }
        if !progressed {
            break;
        }
    }
    out
}

fn matches_terms(text: &str, terms: &[String]) -> bool {
    !terms.is_empty() && matches_terms_lower(&text.to_lowercase(), terms)
}

fn matches_terms_lower(text_lower: &str, terms: &[String]) -> bool {
    !terms.is_empty() && terms.iter().any(|term| text_lower.contains(term))
}

fn matches_preset_keywords_lower(text_lower: &str, topics: &[RecommendationTopic]) -> bool {
    !topics.is_empty()
        && topics.iter().any(|topic| {
            topic
                .keywords()
                .iter()
                .any(|keyword| matches_keyword_boundary(text_lower, keyword))
        })
}

fn matches_keyword_boundary(text_lower: &str, keyword: &str) -> bool {
    let require_end_boundary = keyword.chars().count() <= 3 || keyword.contains(' ');
    let mut offset = 0;
    while let Some(pos) = text_lower[offset..].find(keyword) {
        let start = offset + pos;
        let end = start + keyword.len();
        let before_ok = text_lower[..start]
            .chars()
            .next_back()
            .is_none_or(|c| !c.is_alphanumeric());
        let after_ok = !require_end_boundary
            || text_lower[end..]
                .chars()
                .next()
                .is_none_or(|c| !c.is_alphanumeric());
        if before_ok && after_ok {
            return true;
        }
        let step = text_lower[start..].chars().next().map_or(1, char::len_utf8);
        offset = start + step;
    }
    false
}

fn is_candidate(
    video: &Video,
    taste: &Taste,
    duration: RecommendationDuration,
    muted: &[String],
) -> bool {
    !video.short
        && matches_duration(video, duration)
        && !matches_terms(&video.title, muted)
        && !taste.watched.contains(&video.id)
        && !taste.hidden_videos.contains(&video.id)
        && !video
            .channel_id
            .as_deref()
            .is_some_and(|id| taste.blocked_channels.contains(id))
}

fn channel_multiplier(video: &Video, taste: &Taste, channels: RecommendationChannels) -> f64 {
    let liked = video
        .channel_id
        .as_ref()
        .and_then(|channel| taste.channels.get(channel))
        .copied()
        .unwrap_or(0.0);
    match channels {
        RecommendationChannels::Balanced => 1.0 + 0.5 * liked,
        RecommendationChannels::Discover if video.channel_id.is_some() && liked == 0.0 => 2.0,
        RecommendationChannels::Discover if liked > 0.0 => 0.35,
        RecommendationChannels::Discover => 1.0,
        RecommendationChannels::Familiar if liked > 0.0 => 1.0 + 2.0 * liked,
        RecommendationChannels::Familiar => 0.65,
    }
}

fn matches_topic_text(text: &str, topics: &[RecommendationTopic], interests: &[String]) -> bool {
    if topics.is_empty() && interests.is_empty() {
        return false;
    }
    let lower = text.to_lowercase();
    matches_terms_lower(&lower, interests) || matches_preset_keywords_lower(&lower, topics)
}

fn custom_interest_multiplier(video: &Video, interests: &[String]) -> f64 {
    topic_multiplier(video, &[], interests)
}

fn topic_multiplier(video: &Video, topics: &[RecommendationTopic], interests: &[String]) -> f64 {
    let matched = matches_topic_text(&video.title, topics, interests)
        || video
            .channel
            .as_deref()
            .is_some_and(|channel| matches_topic_text(channel, topics, interests));
    if matched { 2.0 } else { 1.0 }
}

fn matches_duration(video: &Video, duration: RecommendationDuration) -> bool {
    let Some(seconds) = video
        .duration
        .filter(|&seconds| !video.live && seconds.is_finite() && seconds > 0.0)
    else {
        return duration == RecommendationDuration::Any;
    };
    match duration {
        RecommendationDuration::Any => true,
        RecommendationDuration::Short => seconds < 300.0,
        RecommendationDuration::Medium => (300.0..=1200.0).contains(&seconds),
        RecommendationDuration::Long => seconds > 1200.0,
    }
}

fn priority_multiplier(video: &Video, priority: RecommendationPriority, now: i64) -> f64 {
    match priority {
        RecommendationPriority::Balanced => 1.0,
        RecommendationPriority::Recent => {
            video
                .published_at
                .filter(|&date| date <= now)
                .map_or(0.7, |date| {
                    let age_days = now.saturating_sub(date) as f64 / 86_400.0;
                    0.5 + 2.5 / (1.0 + age_days / 14.0)
                })
        }
        RecommendationPriority::FewerViews => video.views.map_or(0.8, |views| {
            let log_views = (views as f64 + 10.0).log10();
            (3.1 - 0.36 * log_views).clamp(0.45, 2.8)
        }),
    }
}

/// The ranking with no channel taking more than [`PER_CHANNEL`] places of
/// each stretch: a channel's later videos wait for the next round.
fn spread(ranked: Vec<Video>) -> Vec<Video> {
    let mut rounds: Vec<Vec<Video>> = Vec::new();
    let mut taken: HashMap<String, usize> = HashMap::new();
    for video in ranked {
        // A video whose channel is unknown is its own: grouping them all
        // as one would push most of them down.
        let channel = video.channel_id.clone().unwrap_or_else(|| video.id.clone());
        let count = taken.entry(channel).or_default();
        let round = *count / PER_CHANNEL;
        *count += 1;
        if rounds.len() <= round {
            rounds.resize_with(round + 1, Vec::new);
        }
        rounds[round].push(video);
    }
    rounds.into_iter().flatten().take(KEPT).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn video(id: &str, channel: &str) -> Video {
        let mut video = Video::from_id(id.to_owned());
        video.channel_id = Some(channel.to_owned());
        video
    }

    fn watched(id: &str, channel: &str, position: f64) -> Watched {
        let mut watched_video = video(id, channel);
        watched_video.duration = Some(100.0);
        Watched {
            video: watched_video,
            at: 0,
            position,
        }
    }

    #[test]
    fn recent_and_finished_videos_seed_the_most() {
        let mut short = watched("s", "UCa", 0.0);
        short.video.short = true;
        let history = [
            watched("a", "UCa", 0.0),
            short,
            watched("b", "UCb", 10.0),
            watched("a", "UCa", 0.0),
            watched("c", "UCc", 0.0),
        ];
        let seeds = seeds(&history);
        let ids: Vec<&str> = seeds.iter().map(|seed| seed.id.as_str()).collect();
        assert_eq!(ids, ["a", "b", "c"]);
        // Barely watched counts less than finished, later as well.
        assert!(seeds[0].weight > seeds[1].weight);
        assert!(seeds[2].weight > seeds[1].weight);
    }

    #[test]
    fn related_videos_rank_by_place_agreement_and_channel() {
        let taste = Taste {
            watched: HashSet::from(["seen".to_owned()]),
            channels: HashMap::from([("UCliked".to_owned(), 1.0)]),
            ..Taste::default()
        };
        let related = [
            (
                1.0,
                vec![
                    video("seen", "UCx"),
                    video("first", "UCx"),
                    video("both", "UCy"),
                    video("liked", "UCliked"),
                ],
            ),
            (1.0, vec![video("both", "UCy")]),
        ];
        let ranked: Vec<String> = rank(&related, &taste)
            .into_iter()
            .map(|video| video.id)
            .collect();
        // Listed twice beats first place; a liked channel beats its place.
        assert_eq!(ranked, ["both", "liked", "first"]);
    }

    #[test]
    fn no_channel_takes_over_the_top() {
        let related = [(
            1.0,
            vec![
                video("a1", "UCa"),
                video("a2", "UCa"),
                video("a3", "UCa"),
                video("b1", "UCb"),
            ],
        )];
        let ranked: Vec<String> = rank(&related, &Taste::default())
            .into_iter()
            .map(|video| video.id)
            .collect();
        assert_eq!(ranked, ["a1", "a2", "b1", "a3"]);
        // Unknown channels are not one channel.
        let unknown = |id: &str| {
            let mut video = video(id, "");
            video.channel_id = None;
            video
        };
        let related = [(1.0, vec![unknown("x1"), unknown("x2"), unknown("x3")])];
        let ranked: Vec<String> = rank(&related, &Taste::default())
            .into_iter()
            .map(|video| video.id)
            .collect();
        assert_eq!(ranked, ["x1", "x2", "x3"]);
    }

    #[test]
    fn duration_and_muted_title_filter_before_ranking() {
        let make = |id: &str, title: &str, duration| Video {
            title: title.into(),
            duration,
            ..video(id, "UCa")
        };
        let related = [(
            1.0,
            vec![
                make("muted", "ÁUDIO promocional", Some(600.0)),
                make("unknown", "Unknown", None),
                make("short", "Short", Some(299.0)),
                make("five", "Five", Some(300.0)),
                make("twenty", "Twenty", Some(1200.0)),
                make("long", "Long", Some(1201.0)),
            ],
        )];
        let preferences = Recommendations {
            duration: RecommendationDuration::Medium,
            muted: " , áudio , ".into(),
            ..Recommendations::default()
        };
        let ids: Vec<_> = rank_with(&related, &Taste::default(), &preferences, 0)
            .into_iter()
            .map(|video| video.id)
            .collect();
        assert_eq!(ids, ["five", "twenty"]);
    }

    #[test]
    fn discovery_and_familiarity_choose_different_channels() {
        let related = [(
            1.0,
            vec![
                video("known", "UCa"),
                video("new1", "UCb"),
                video("new2", "UCb"),
            ],
        )];
        let taste = Taste {
            channels: HashMap::from([("UCa".into(), 1.0)]),
            ..Taste::default()
        };
        let mut preferences = Recommendations {
            channels: RecommendationChannels::Discover,
            ..Recommendations::default()
        };
        let ids: Vec<_> = rank_with(&related, &taste, &preferences, 0)
            .into_iter()
            .map(|video| video.id)
            .collect();
        assert_eq!(ids, ["new1", "known"]);
        preferences.channels = RecommendationChannels::Familiar;
        assert_eq!(rank_with(&related, &taste, &preferences, 0)[0].id, "known");
    }

    #[test]
    fn interest_recency_and_low_view_bonuses_do_not_reward_missing_metadata() {
        let now = 30 * 86_400;
        let mut chosen = video("chosen", "UCb");
        chosen.channel = Some("Tudo sobre CIÊNCIA".into());
        chosen.published_at = Some(now - 86_400);
        chosen.views = Some(200);
        let related = [(1.0, vec![video("missing", "UCa"), chosen])];
        for preferences in [
            Recommendations {
                interests: "ciência".into(),
                ..Recommendations::default()
            },
            Recommendations {
                priority: RecommendationPriority::Recent,
                ..Recommendations::default()
            },
            Recommendations {
                priority: RecommendationPriority::FewerViews,
                ..Recommendations::default()
            },
        ] {
            assert_eq!(
                rank_with(&related, &Taste::default(), &preferences, now)[0].id,
                "chosen"
            );
        }
    }

    #[test]
    fn subscriptions_count_fully_in_the_taste() {
        let history = [
            watched("a", "UCa", 0.0),
            watched("b", "UCa", 0.0),
            watched("c", "UCc", 0.0),
        ];
        let subscriptions = [Channel {
            id: "UCs".to_owned(),
            name: "S".to_owned(),
            avatar: None,
        }];
        let taste = taste(&history, &subscriptions);
        assert_eq!(taste.channels["UCa"], 1.0);
        assert_eq!(taste.channels["UCc"], 0.5);
        assert_eq!(taste.channels["UCs"], 1.0);
        assert!(taste.watched.contains("b"));
    }

    #[test]
    fn hidden_videos_and_blocked_channels_are_excluded_from_seeds_and_rank() {
        let mut library = Library::default();
        let v_hidden = video("hidden00001", "UCok");
        let v_blocked = video("from_block1", "UCbad");
        let v_ok = video("ok000000001", "UCok");
        library.record_watch(v_ok.clone(), 1);
        library.record_watch(v_blocked.clone(), 2);
        library.record_watch(v_hidden.clone(), 3);
        library.hide_video("hidden00001".into());
        library.hide_video("unwatched01".into());
        library.block_channel(Channel {
            id: "UCbad".into(),
            name: "Bad Channel".into(),
            avatar: None,
        });

        let seeds = seeds_for_library(&library);
        let seed_ids: Vec<&str> = seeds.iter().map(|seed| seed.id.as_str()).collect();
        assert_eq!(seed_ids, ["ok000000001"]);

        let taste = taste_for_library(&library);
        assert!(!taste.channels.contains_key("UCbad"));
        let related = [(
            1.0,
            vec![
                video("hidden00001", "UCok"),
                video("unwatched01", "UCok"),
                video("from_block1", "UCbad"),
                video("other_bad01", "UCbad"),
                video("fresh_good1", "UCgood"),
            ],
        )];
        let ranked: Vec<String> = rank(&related, &taste)
            .into_iter()
            .map(|video| video.id)
            .collect();
        assert_eq!(ranked, ["fresh_good1"]);
    }

    #[test]
    fn excluded_watches_do_not_take_affinity_slots_or_change_weights() {
        let mut library = Library::default();
        let blocked = Channel {
            id: "UCblocked".into(),
            name: "Blocked".into(),
            avatar: None,
        };
        library.subscriptions.push(blocked.clone());
        assert!(library.block_channel(blocked));
        assert!(library.hide_video("hidden-watch".into()));
        library.history.push(watched("hidden-watch", "UCside", 0.0));
        library.history.extend(
            (0..AFFINITY_WATCHES).map(|i| watched(&format!("blocked-{i}"), "UCblocked", 0.0)),
        );
        library.history.extend([
            watched("first", "UCgood", 0.0),
            watched("second", "UCgood", 0.0),
            watched("third", "UCside", 0.0),
        ]);

        let taste = taste_for_library(&library);
        assert_eq!(taste.channels.len(), 2);
        assert_eq!(taste.channels["UCgood"], 1.0);
        assert_eq!(taste.channels["UCside"], 0.5);
        assert!(taste.watched.contains("hidden-watch"));
        assert!(taste.watched.contains("blocked-0"));
    }

    #[test]
    fn preset_topics_interleave_with_history_without_duplicates() {
        let history = [
            (
                1.0,
                vec![
                    video("clash1", "UCclash"),
                    video("clash2", "UCclash"),
                    video("overlap", "UCtech"),
                ],
            ),
            (0.9, vec![video("clash1", "UCclash")]),
            (0.8, vec![video("clash1", "UCclash")]),
        ];
        let topics = [vec![
            video("tech1", "UCtech1"),
            video("overlap", "UCtech"),
            video("tech2", "UCtech2"),
        ]];
        let preferences = Recommendations {
            topics: vec![RecommendationTopic::Computing],
            ..Recommendations::default()
        };
        let ids: Vec<String> = rank_combined(&history, &topics, &Taste::default(), &preferences, 0)
            .into_iter()
            .map(|video| video.id)
            .collect();
        // Topic and history alternate cleanly and duplicate video IDs appear once.
        assert_eq!(ids, ["tech1", "clash1", "overlap", "clash2", "tech2"]);
    }

    #[test]
    fn discover_penalizes_multi_seed_familiar_channels() {
        let history = [
            (1.0, vec![video("fam", "UCfam"), video("fresh", "UCnew")]),
            (0.9, vec![video("fam", "UCfam")]),
            (0.8, vec![video("fam", "UCfam")]),
        ];
        let taste = Taste {
            channels: HashMap::from([("UCfam".into(), 1.0)]),
            ..Taste::default()
        };
        let preferences = Recommendations {
            channels: RecommendationChannels::Discover,
            ..Recommendations::default()
        };
        let ids: Vec<String> = rank_with(&history, &taste, &preferences, 0)
            .into_iter()
            .map(|video| video.id)
            .collect();
        assert_eq!(ids, ["fresh", "fam"]);
    }

    #[test]
    fn short_preset_keywords_require_word_boundaries() {
        let mut false_positive = video("daily", "UCa");
        false_positive.title = "Daily main rain train".into();
        let mut real_ai = video("real_ai", "UCb");
        real_ai.title = "Building local AI agents".into();
        let history = [(1.0, vec![false_positive, real_ai])];
        let preferences = Recommendations {
            topics: vec![RecommendationTopic::Computing],
            ..Recommendations::default()
        };
        assert_eq!(
            rank_with(&history, &Taste::default(), &preferences, 0)[0].id,
            "real_ai"
        );
    }

    #[test]
    fn fewer_views_and_recent_overcome_multi_seed_bias() {
        let now = 365 * 86_400;
        let mut viral_old = video("viral_old", "UCa");
        viral_old.views = Some(8_000_000);
        viral_old.published_at = Some(now - 300 * 86_400);
        let mut small_recent = video("small_recent", "UCb");
        small_recent.views = Some(5_000);
        small_recent.published_at = Some(now - 2 * 86_400);
        let history = [
            (1.0, vec![viral_old.clone(), small_recent]),
            (0.9, vec![viral_old.clone()]),
            (0.8, vec![viral_old]),
        ];
        for priority in [
            RecommendationPriority::Recent,
            RecommendationPriority::FewerViews,
        ] {
            let preferences = Recommendations {
                priority,
                ..Recommendations::default()
            };
            assert_eq!(
                rank_with(&history, &Taste::default(), &preferences, now)[0].id,
                "small_recent",
                "priority {priority:?} should promote small_recent over multi-seed viral_old"
            );
        }
    }

    #[test]
    fn interleave_enforces_per_channel_limit_in_balanced_and_familiar() {
        let history = [(
            1.0,
            vec![
                video("h1", "UCsame"),
                video("h2", "UCsame"),
                video("h_other", "UCother"),
            ],
        )];
        let topics = [
            vec![video("t1", "UCsame"), video("t2", "UCsame")],
            vec![video("t3", "UCsame"), video("t_other", "UCtopic")],
        ];
        for channels in [
            RecommendationChannels::Balanced,
            RecommendationChannels::Familiar,
        ] {
            let preferences = Recommendations {
                channels,
                ..Recommendations::default()
            };
            let ranked = rank_combined(&history, &topics, &Taste::default(), &preferences, 0);
            let top_same = ranked
                .iter()
                .take(4)
                .filter(|v| v.channel_id.as_deref() == Some("UCsame"))
                .count();
            assert_eq!(
                top_same, PER_CHANNEL,
                "channel cap violated in {channels:?}: {ranked:?}"
            );
        }
    }

    #[test]
    fn rank_topics_boosts_custom_interests_and_handles_empty_buckets() {
        let mut generic = video("generic", "UCa");
        generic.title = "General programming news".into();
        let mut rust_video = video("rust_vid", "UCb");
        rust_video.title = "Writing fast Rust code".into();
        let history = [(1.0, vec![video("hist1", "UCh1"), video("hist2", "UCh2")])];
        // Second topic bucket is empty, so active_topics == 1 (1:1 interleave).
        let topics = [vec![generic, rust_video], Vec::new()];
        let preferences = Recommendations {
            interests: "rust".into(),
            ..Recommendations::default()
        };
        let ids: Vec<String> = rank_combined(&history, &topics, &Taste::default(), &preferences, 0)
            .into_iter()
            .map(|v| v.id)
            .collect();
        assert_eq!(ids, ["rust_vid", "hist1", "generic", "hist2"]);
    }

    #[test]
    fn discover_interleave_keeps_unknown_channels_and_dedups_known_ones() {
        let mut no_ch_1 = video("anon1", "");
        no_ch_1.channel_id = None;
        let mut no_ch_2 = video("anon2", "");
        no_ch_2.channel_id = None;
        let history = [(1.0, vec![video("h_dup", "UCx"), no_ch_2])];
        let topics = [vec![video("t_dup", "UCx"), no_ch_1]];
        let preferences = Recommendations {
            channels: RecommendationChannels::Discover,
            ..Recommendations::default()
        };
        let ids: Vec<String> = rank_combined(&history, &topics, &Taste::default(), &preferences, 0)
            .into_iter()
            .map(|v| v.id)
            .collect();
        // UCx appears only once across lanes, while ownerless videos remain distinct.
        assert_eq!(ids, ["t_dup", "anon2", "anon1"]);
    }

    #[test]
    fn empty_history_with_multiple_topics_spreads_across_topics() {
        let topics = [
            vec![video("a1", "UCa"), video("a2", "UCa2")],
            vec![video("b1", "UCb"), video("b2", "UCb2")],
        ];
        let ids: Vec<String> = rank_combined(
            &[],
            &topics,
            &Taste::default(),
            &Recommendations::default(),
            0,
        )
        .into_iter()
        .map(|v| v.id)
        .collect();
        assert_eq!(ids, ["a1", "b1", "a2", "b2"]);
    }
}
