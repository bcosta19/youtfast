//! In-memory candidate pages, before any local filtering or ranking.

use std::collections::VecDeque;
use std::future::Future;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use youtfast_core::model::Video;
use youtfast_youtube::innertube::SearchOptions;

const CANDIDATE_CAPACITY: usize = 64;
const CANDIDATE_TTL: Duration = Duration::from_secs(30 * 60);

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum CandidateKey {
    Seed {
        epoch: u64,
        id: String,
    },
    Topic {
        query: String,
        options: SearchOptions,
    },
}

#[derive(Default)]
pub(super) struct RecommendationCache(Mutex<CandidatePages>);

#[derive(Default)]
struct CandidatePages {
    epoch: u64,
    pages: VecDeque<CandidatePage>,
}

struct CandidatePage {
    key: CandidateKey,
    videos: Vec<Video>,
    stored_at: Instant,
}

impl CandidatePages {
    fn current(&self, key: &CandidateKey) -> bool {
        !matches!(key, CandidateKey::Seed { epoch, .. } if *epoch != self.epoch)
    }

    fn get(&self, key: &CandidateKey, now: Instant) -> Option<Vec<Video>> {
        if !self.current(key) {
            return None;
        }
        self.pages
            .iter()
            .find(|page| &page.key == key && now.duration_since(page.stored_at) < CANDIDATE_TTL)
            .map(|page| page.videos.clone())
    }

    fn insert(&mut self, key: CandidateKey, videos: Vec<Video>, now: Instant) {
        // An aborted old load may still be finishing when the epoch changes.
        if !self.current(&key) {
            return;
        }
        self.pages
            .retain(|page| page.key != key && now.duration_since(page.stored_at) < CANDIDATE_TTL);
        if self.pages.len() == CANDIDATE_CAPACITY {
            self.pages.pop_front();
        }
        self.pages.push_back(CandidatePage {
            key,
            videos,
            stored_at: now,
        });
    }
}

impl RecommendationCache {
    pub(super) fn advance_session(&self, epoch: u64) {
        let mut cache = self.0.lock().unwrap();
        cache.epoch = epoch;
        cache
            .pages
            .retain(|page| matches!(page.key, CandidateKey::Topic { .. }));
    }

    pub(super) fn get(&self, key: &CandidateKey) -> Option<Vec<Video>> {
        self.0.lock().unwrap().get(key, Instant::now())
    }

    pub(super) async fn fetch_and_store<F: Future<Output = Result<Vec<Video>, String>>>(
        &self,
        key: CandidateKey,
        fetch: impl FnOnce(CandidateKey) -> F,
    ) -> Result<Vec<Video>, String> {
        let videos = fetch(key.clone()).await?;
        self.0
            .lock()
            .unwrap()
            .insert(key, videos.clone(), Instant::now());
        Ok(videos)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use youtfast_youtube::innertube::{SearchDate, SearchLength};

    fn seed(epoch: u64, id: impl Into<String>) -> CandidateKey {
        CandidateKey::Seed {
            epoch,
            id: id.into(),
        }
    }

    fn topic(query: &str, date: SearchDate, length: SearchLength) -> CandidateKey {
        CandidateKey::Topic {
            query: query.into(),
            options: SearchOptions {
                date,
                length,
                ..SearchOptions::default()
            },
        }
    }

    #[test]
    fn candidate_cache_expires_at_ttl() {
        let mut pages = CandidatePages::default();
        let key = seed(0, "seed");
        let now = Instant::now();
        pages.insert(key.clone(), Vec::new(), now);
        assert!(
            pages
                .get(&key, now + CANDIDATE_TTL - Duration::from_secs(1))
                .is_some()
        );
        assert!(pages.get(&key, now + CANDIDATE_TTL).is_none());
    }

    #[test]
    fn candidate_cache_evicts_oldest_insert_even_when_recently_read() {
        let mut pages = CandidatePages::default();
        let now = Instant::now();
        for i in 0..CANDIDATE_CAPACITY {
            pages.insert(seed(0, i.to_string()), Vec::new(), now);
        }
        assert!(pages.get(&seed(0, "0"), now).is_some());
        pages.insert(seed(0, "new"), Vec::new(), now);
        assert_eq!(pages.pages.len(), CANDIDATE_CAPACITY);
        assert!(pages.get(&seed(0, "0"), now).is_none());
        assert!(pages.get(&seed(0, "1"), now).is_some());
        assert!(pages.get(&seed(0, "new"), now).is_some());
    }

    #[test]
    fn candidate_cache_separates_topic_query_date_and_duration() {
        let mut pages = CandidatePages::default();
        let now = Instant::now();
        let variants = [
            topic("cats", SearchDate::Month, SearchLength::Short),
            topic("cats", SearchDate::Any, SearchLength::Short),
            topic("cats", SearchDate::Month, SearchLength::Medium),
            topic("dogs", SearchDate::Month, SearchLength::Short),
        ];
        for (i, key) in variants.iter().enumerate() {
            pages.insert(key.clone(), vec![Video::from_id(i.to_string())], now);
        }
        for (i, key) in variants.iter().enumerate() {
            assert_eq!(pages.get(key, now).unwrap()[0].id, i.to_string());
        }
    }

    #[tokio::test]
    async fn candidate_cache_clears_seeds_and_rejects_old_epoch_inserts() {
        let cache = RecommendationCache::default();
        let old_seed = seed(0, "same");
        let new_seed = seed(1, "same");
        let topic = topic("cats", SearchDate::Any, SearchLength::Any);
        for key in [&old_seed, &topic] {
            cache
                .fetch_and_store(key.clone(), |_| std::future::ready(Ok(Vec::new())))
                .await
                .unwrap();
        }
        assert!(cache.get(&old_seed).is_some());
        cache.advance_session(1);
        assert!(
            cache
                .0
                .lock()
                .unwrap()
                .pages
                .iter()
                .all(|page| page.key != old_seed)
        );
        assert!(cache.get(&topic).is_some());
        assert!(cache.get(&new_seed).is_none());
        cache
            .fetch_and_store(old_seed.clone(), |_| std::future::ready(Ok(Vec::new())))
            .await
            .unwrap();
        assert_eq!(cache.0.lock().unwrap().pages.len(), 1);
        cache
            .fetch_and_store(new_seed.clone(), |_| std::future::ready(Ok(Vec::new())))
            .await
            .unwrap();
        assert!(cache.get(&new_seed).is_some());
    }

    #[tokio::test]
    async fn candidate_cache_does_not_keep_errors() {
        let cache = RecommendationCache::default();
        let key = seed(0, "seed");
        assert!(
            cache
                .fetch_and_store(key.clone(), |_| std::future::ready(Err("failed".into())))
                .await
                .is_err()
        );
        assert!(cache.get(&key).is_none());
    }
}
