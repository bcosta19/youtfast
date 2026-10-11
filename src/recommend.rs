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

use crate::library::Watched;
use crate::model::{Channel, Video};

/// How many watched videos seed the list: each costs a details request.
pub const SEEDS: usize = 12;
/// At most this many of one channel in a row of the ranking before the
/// others get their turn.
const PER_CHANNEL: usize = 2;
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
}

/// The seeds: the latest distinct videos of the history, not Shorts or
/// live streams (their related lists are about the moment), weighed by
/// how recent they are and how much of them was watched.
pub fn seeds(history: &[Watched]) -> Vec<Seed> {
    let mut seen = HashSet::new();
    history
        .iter()
        .filter(|watched| !watched.video.short && !watched.video.live)
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
    let mut counts: HashMap<String, f64> = HashMap::new();
    for watched in history.iter().take(200) {
        if let Some(channel) = &watched.video.channel_id {
            *counts.entry(channel.clone()).or_default() += 1.0;
        }
    }
    let most = counts.values().copied().fold(1.0, f64::max);
    let mut channels: HashMap<String, f64> = counts
        .into_iter()
        .map(|(channel, count)| (channel, count / most))
        .collect();
    for channel in subscriptions {
        channels.insert(channel.id.clone(), 1.0);
    }
    Taste {
        watched: history
            .iter()
            .map(|watched| watched.video.id.clone())
            .collect(),
        channels,
    }
}

/// The ranked list from each seed's weight and its related videos, in
/// YouTube's order.
pub fn rank(related: &[(f64, Vec<Video>)], taste: &Taste) -> Vec<Video> {
    let mut scores: HashMap<String, (f64, Video)> = HashMap::new();
    for (weight, videos) in related {
        for (place, video) in videos.iter().enumerate() {
            if video.short || taste.watched.contains(&video.id) {
                continue;
            }
            let score = weight / (1.0 + place as f64 * 0.15);
            scores
                .entry(video.id.clone())
                .and_modify(|(total, _)| *total += score)
                .or_insert((score, video.clone()));
        }
    }
    let mut ranked: Vec<(f64, Video)> = scores
        .into_values()
        .map(|(score, video)| {
            let liked = video
                .channel_id
                .as_ref()
                .and_then(|channel| taste.channels.get(channel))
                .copied()
                .unwrap_or(0.0);
            (score * (1.0 + 0.5 * liked), video)
        })
        .collect();
    ranked.sort_by(|a, b| b.0.total_cmp(&a.0).then_with(|| a.1.id.cmp(&b.1.id)));
    spread(ranked.into_iter().map(|(_, video)| video).collect())
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
}
