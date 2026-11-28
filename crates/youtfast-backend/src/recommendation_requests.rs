//! Shared request budget for local recommendation candidate fetches.

use std::future::Future;
use std::time::Duration;

use tokio::sync::{Mutex, Semaphore};
use tokio::time::Instant;

const MAX_IN_FLIGHT: usize = 3;
const START_SPACING: Duration = Duration::from_millis(200);

pub(super) struct RecommendationRequests {
    turns: Semaphore,
    last_start: Mutex<Option<Instant>>,
}

impl Default for RecommendationRequests {
    fn default() -> Self {
        Self {
            turns: Semaphore::new(MAX_IN_FLIGHT),
            last_start: Mutex::new(None),
        }
    }
}

impl RecommendationRequests {
    pub(super) async fn fetch<T>(
        &self,
        latest_start: Instant,
        request: impl Future<Output = Result<T, String>>,
    ) -> Result<T, String> {
        let acquire_turn = async {
            let turn = self.turns.acquire().await.expect("semaphore never closes");
            let mut last_start = self.last_start.lock().await;
            if let Some(last_start) = *last_start {
                tokio::time::sleep_until(last_start + START_SPACING).await;
            }
            let now = Instant::now();
            if now >= latest_start {
                return Err(super::LOAD_OVERDUE.to_owned());
            }
            *last_start = Some(now);
            Ok(turn)
        };
        let _turn = tokio::time::timeout_at(latest_start, acquire_turn)
            .await
            .map_err(|_| super::LOAD_OVERDUE.to_owned())??;
        request.await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[tokio::test]
    async fn revoked_topic_search_waiting_for_a_turn_never_starts() {
        use crate::{CandidateLookup, TopicSearchConsent, search_topic_pages};
        use youtfast_youtube::innertube::SearchOptions;

        let requests = RecommendationRequests::default();
        let all_turns = requests
            .turns
            .acquire_many(MAX_IN_FLIGHT as u32)
            .await
            .unwrap();
        let consent = TopicSearchConsent::granted();
        let candidates = CandidateLookup::default();
        let search = search_topic_pages(
            &candidates,
            "synthetic",
            SearchOptions::default(),
            &consent,
            |videos| !videos.is_empty(),
            |_| {
                requests.fetch(Instant::now() + Duration::from_secs(5), async {
                    panic!("revoked search started")
                })
            },
        );
        tokio::pin!(search);
        tokio::select! {
            biased;
            _ = &mut search => panic!("search should wait for a turn"),
            () = tokio::task::yield_now() => {}
        }
        consent.clone().revoke();
        drop(all_turns);
        assert!(search.await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn recommendation_requests_limit_in_flight_and_space_starts() {
        let requests = Arc::new(RecommendationRequests::default());
        let (starts, mut started) = tokio::sync::mpsc::unbounded_channel();
        let in_flight = Arc::new(AtomicUsize::new(0));
        let finish = Arc::new(Semaphore::new(0));
        let mut tasks = tokio::task::JoinSet::new();
        for _ in 0..5 {
            let (requests, starts, in_flight, finish) = (
                requests.clone(),
                starts.clone(),
                in_flight.clone(),
                finish.clone(),
            );
            tasks.spawn(async move {
                requests
                    .fetch(Instant::now() + Duration::from_secs(5), async {
                        let active = in_flight.fetch_add(1, Ordering::SeqCst) + 1;
                        assert!(active <= 3);
                        starts.send((Instant::now(), active)).unwrap();
                        finish.acquire().await.unwrap().forget();
                        in_flight.fetch_sub(1, Ordering::SeqCst);
                        Ok(())
                    })
                    .await
                    .unwrap();
            });
        }
        drop(starts);
        tokio::time::timeout(Duration::from_secs(5), async {
            let mut previous = None;
            for expected in 1..=3 {
                let (start, active) = started.recv().await.unwrap();
                assert_eq!(active, expected);
                if let Some(previous) = previous {
                    // Allow for the time between recording the start and polling the request.
                    assert!(start - previous >= START_SPACING - Duration::from_millis(10));
                }
                previous = Some(start);
            }
            assert!(
                tokio::time::timeout(START_SPACING * 2, started.recv())
                    .await
                    .is_err()
            );
            finish.add_permits(5);
            while let Some(result) = tasks.join_next().await {
                result.unwrap();
            }
        })
        .await
        .unwrap();
    }
}
