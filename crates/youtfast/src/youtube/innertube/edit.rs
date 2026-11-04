//! What the app changes on the account: a video's rating and the
//! playlists that hold it. rustypipe reads the account but has no write
//! calls, so these go through its `raw` request (signed in, as the web
//! client) to YouTube's own endpoints, and read back only the fields they
//! need.

use rustypipe::client::{ClientType, RustyPipe};
use serde::Serialize;
use serde_json::{Value, json};

use crate::model::{Rating, SaveTarget};

/// Sends `body` to an InnerTube endpoint as the signed-in web client.
pub(super) async fn post(
    rp: &RustyPipe,
    endpoint: &str,
    body: &impl Serialize,
) -> Result<Value, String> {
    post_as(rp, endpoint, body, true).await
}

/// [`post`], signed in or not: a public list needs no session.
pub(super) async fn post_as(
    rp: &RustyPipe,
    endpoint: &str,
    body: &impl Serialize,
    authenticated: bool,
) -> Result<Value, String> {
    let query = if authenticated {
        rp.query().authenticated()
    } else {
        rp.query()
    };
    let text = query
        .raw(ClientType::Desktop, endpoint, body)
        .await
        .map_err(|error| error.to_string())?;
    serde_json::from_str(&text).map_err(|error| format!("YouTube's answer was unreadable: {error}"))
}

/// The account's rating of a video, from the watch page's data.
pub async fn rating(rp: &RustyPipe, id: &str) -> Result<Rating, String> {
    let page = post(rp, "next", &json!({ "videoId": id })).await?;
    Ok(rating_in(&page))
}

/// Likes, dislikes, or clears the account's rating of a video.
pub async fn rate(rp: &RustyPipe, id: &str, rating: Rating) -> Result<(), String> {
    let endpoint = match rating {
        Rating::Like => "like/like",
        Rating::Dislike => "like/dislike",
        Rating::None => "like/removelike",
    };
    post(rp, endpoint, &json!({ "target": { "videoId": id } }))
        .await
        .map(drop)
}

/// The account's playlists, Watch later first, each with whether it holds
/// the video: what YouTube's own save menu lists.
pub async fn save_targets(rp: &RustyPipe, id: &str) -> Result<Vec<SaveTarget>, String> {
    let body = json!({ "videoIds": [id], "excludeWatchLater": false });
    let answer = post(rp, "playlist/get_add_to_playlist", &body).await?;
    let targets = save_targets_in(&answer);
    if targets.is_empty() {
        return Err("YouTube listed no playlists to save to".to_owned());
    }
    Ok(targets)
}

/// Adds the video to one of the account's playlists, or takes it out.
pub async fn set_saved(
    rp: &RustyPipe,
    id: &str,
    playlist: &str,
    saved: bool,
) -> Result<(), String> {
    let action = if saved {
        json!({ "action": "ACTION_ADD_VIDEO", "addedVideoId": id })
    } else {
        json!({ "action": "ACTION_REMOVE_VIDEO_BY_VIDEO_ID", "removedVideoId": id })
    };
    let body = json!({ "playlistId": playlist, "actions": [action] });
    let answer = post(rp, "browse/edit_playlist", &body).await?;
    match answer.get("status").and_then(Value::as_str) {
        None | Some("STATUS_SUCCEEDED") => Ok(()),
        Some(status) => Err(format!("YouTube did not change the playlist ({status})")),
    }
}

/// The rating in a `next` answer: the like button's entity, wherever the
/// layout of the day puts it. None found reads as no rating.
fn rating_in(page: &Value) -> Rating {
    let status = find(page, &|value| {
        value
            .get("likeStatusEntity")
            .and_then(|entity| entity.get("likeStatus"))
            .or_else(|| value.get("likeStatus"))
            .and_then(Value::as_str)
    });
    match status {
        Some("LIKE") => Rating::Like,
        Some("DISLIKE") => Rating::Dislike,
        _ => Rating::None,
    }
}

/// The playlists a `get_add_to_playlist` answer offers.
fn save_targets_in(answer: &Value) -> Vec<SaveTarget> {
    let mut targets = Vec::new();
    collect(answer, &mut |value| {
        let Some(option) = value.get("playlistAddToOptionRenderer") else {
            return;
        };
        let Some(id) = option.get("playlistId").and_then(Value::as_str) else {
            return;
        };
        targets.push(SaveTarget {
            id: id.to_owned(),
            name: text(option.get("title")).unwrap_or_else(|| id.to_owned()),
            saved: option.get("containsSelectedVideos").and_then(Value::as_str) == Some("ALL"),
        });
    });
    targets
}

/// YouTube's text object: `simpleText`, or `runs` joined.
pub(super) fn text(value: Option<&Value>) -> Option<String> {
    let value = value?;
    if let Some(simple) = value.get("simpleText").and_then(Value::as_str) {
        return Some(simple.to_owned());
    }
    let runs = value.get("runs")?.as_array()?;
    Some(
        runs.iter()
            .filter_map(|run| run.get("text").and_then(Value::as_str))
            .collect(),
    )
}

/// The first answer `pick` gives, searching depth first.
pub(super) fn find<'a, T>(value: &'a Value, pick: &impl Fn(&'a Value) -> Option<T>) -> Option<T> {
    if let Some(found) = pick(value) {
        return Some(found);
    }
    match value {
        Value::Object(map) => map.values().find_map(|child| find(child, pick)),
        Value::Array(items) => items.iter().find_map(|child| find(child, pick)),
        _ => None,
    }
}

/// Shows `visit` every object and array, depth first, in document order.
pub(super) fn collect<'a>(value: &'a Value, visit: &mut impl FnMut(&'a Value)) {
    visit(value);
    match value {
        Value::Object(map) => map.values().for_each(|child| collect(child, visit)),
        Value::Array(items) => items.iter().for_each(|child| collect(child, visit)),
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_rating_is_read_from_the_like_entity() {
        let page = json!({
            "contents": { "twoColumnWatchNextResults": {} },
            "frameworkUpdates": { "entityBatchUpdate": { "mutations": [
                { "payload": { "likeCountEntity": { "likeCountIfLiked": {} } } },
                { "payload": { "likeStatusEntity": { "key": "x", "likeStatus": "DISLIKE" } } }
            ] } }
        });
        assert_eq!(rating_in(&page), Rating::Dislike);
        let older = json!({ "buttons": [{ "likeStatus": "LIKE" }] });
        assert_eq!(rating_in(&older), Rating::Like);
        let unrated = json!({ "likeStatusEntity": { "likeStatus": "INDIFFERENT" } });
        assert_eq!(rating_in(&unrated), Rating::None);
        assert_eq!(rating_in(&json!({})), Rating::None);
    }

    #[test]
    fn save_targets_keep_order_names_and_membership() {
        let answer = json!({ "contents": [{ "addToPlaylistRenderer": { "playlists": [
            { "playlistAddToOptionRenderer": {
                "playlistId": "WL",
                "title": { "simpleText": "Watch later" },
                "containsSelectedVideos": "ALL"
            } },
            { "playlistAddToOptionRenderer": {
                "playlistId": "PL1",
                "title": { "runs": [{ "text": "Rust " }, { "text": "talks" }] },
                "containsSelectedVideos": "NONE"
            } }
        ] } }] });
        assert_eq!(
            save_targets_in(&answer),
            vec![
                SaveTarget {
                    id: "WL".to_owned(),
                    name: "Watch later".to_owned(),
                    saved: true,
                },
                SaveTarget {
                    id: "PL1".to_owned(),
                    name: "Rust talks".to_owned(),
                    saved: false,
                },
            ]
        );
    }
}
