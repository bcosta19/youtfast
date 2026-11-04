//! YouTube links pasted, opened or imported: which video or channel they
//! name. Pure parsing, no network.

use crate::model::Channel;

pub fn is_video_id(id: &str) -> bool {
    id.len() == 11
        && id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_')
}

fn is_channel_id(id: &str) -> bool {
    id.len() == 24
        && id.starts_with("UC")
        && id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_')
}

fn is_channel_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 100
        && name
            .chars()
            .all(|c| c.is_alphanumeric() || matches!(c, '_' | '-' | '.' | '·' | '%'))
}

fn valid_channel_tail(tail: &str) -> bool {
    matches!(
        tail,
        "" | "featured"
            | "videos"
            | "shorts"
            | "streams"
            | "live"
            | "playlists"
            | "about"
            | "community"
            | "posts"
            | "podcasts"
            | "releases"
            | "channels"
    )
}

/// The input after an optional `http(s)://` and `www.` / `m.` / `music.`.
fn strip_host_prefix(input: &str) -> &str {
    let input = input.trim();
    let rest = input
        .strip_prefix("https://")
        .or_else(|| input.strip_prefix("http://"))
        .unwrap_or(input);
    let rest = rest
        .strip_prefix("www.")
        .or_else(|| rest.strip_prefix("m."))
        .unwrap_or(rest);
    rest.strip_prefix("music.").unwrap_or(rest)
}

/// The video id in a YouTube link, so pasting one opens it instead of
/// searching for it. Bare ids are not accepted: an 11-letter word such as
/// "programming" would be one.
pub fn video_id_from_link(input: &str) -> Option<String> {
    let rest = strip_host_prefix(input);

    let candidate = if let Some(path) = rest.strip_prefix("youtu.be/") {
        path
    } else {
        let path = rest.strip_prefix("youtube.com/")?;
        if let Some(query) = path.strip_prefix("watch?") {
            query.split('&').find_map(|pair| pair.strip_prefix("v="))?
        } else {
            ["shorts/", "live/", "embed/", "v/"]
                .iter()
                .find_map(|prefix| path.strip_prefix(prefix))?
        }
    };
    let id: String = candidate
        .chars()
        .take_while(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_')
        .collect();
    is_video_id(&id).then_some(id)
}

/// The channel in a YouTube channel link (`/@handle`, `/channel/UC…`,
/// `/c/name`, `/user/name`), so pasting or opening one opens its page
/// instead of searching for the URL. A handle or vanity path keeps that
/// path as `id` until the channel header resolves it to `UC…`.
pub fn channel_from_link(input: &str) -> Option<Channel> {
    let path = strip_host_prefix(input).strip_prefix("youtube.com/")?;
    let path = path
        .split(['?', '#'])
        .next()
        .unwrap_or(path)
        .trim_end_matches('/');

    if let Some(rest) = path.strip_prefix("channel/") {
        let (id, tail) = rest.split_once('/').unwrap_or((rest, ""));
        return (is_channel_id(id) && valid_channel_tail(tail)).then(|| Channel {
            id: id.to_owned(),
            name: String::new(),
            avatar: None,
        });
    }
    if let Some(rest) = path.strip_prefix('@') {
        let (handle, tail) = rest.split_once('/').unwrap_or((rest, ""));
        return (is_channel_name(handle) && valid_channel_tail(tail)).then(|| {
            let handle = format!("@{handle}");
            Channel {
                id: handle.clone(),
                name: handle,
                avatar: None,
            }
        });
    }
    for prefix in ["c/", "user/"] {
        if let Some(rest) = path.strip_prefix(prefix) {
            let (name, tail) = rest.split_once('/').unwrap_or((rest, ""));
            return (is_channel_name(name) && valid_channel_tail(tail)).then(|| Channel {
                id: format!("{prefix}{name}"),
                name: name.to_owned(),
                avatar: None,
            });
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn video_links() {
        let id = Some("dQw4w9WgXcQ".to_owned());
        for link in [
            "https://www.youtube.com/watch?v=dQw4w9WgXcQ",
            "https://youtube.com/watch?feature=share&v=dQw4w9WgXcQ&t=42",
            "http://m.youtube.com/watch?v=dQw4w9WgXcQ",
            "https://music.youtube.com/watch?v=dQw4w9WgXcQ&list=RD",
            "youtube.com/watch?v=dQw4w9WgXcQ",
            "https://youtu.be/dQw4w9WgXcQ?si=abc",
            "https://www.youtube.com/shorts/dQw4w9WgXcQ",
            "https://www.youtube.com/live/dQw4w9WgXcQ?feature=share",
            "https://www.youtube.com/embed/dQw4w9WgXcQ",
            "  https://youtu.be/dQw4w9WgXcQ  ",
        ] {
            assert_eq!(video_id_from_link(link), id, "{link}");
        }
    }

    #[test]
    fn channel_links() {
        for link in [
            "https://www.youtube.com/@BlenderStudio",
            "https://youtube.com/@BlenderStudio/",
            "http://m.youtube.com/@BlenderStudio/videos",
            "https://www.youtube.com/@BlenderStudio/shorts?si=abc",
            "youtube.com/@BlenderStudio#about",
            "  https://www.youtube.com/@BlenderStudio/playlists  ",
        ] {
            assert_eq!(
                channel_from_link(link),
                Some(Channel {
                    id: "@BlenderStudio".to_owned(),
                    name: "@BlenderStudio".to_owned(),
                    avatar: None,
                }),
                "{link}"
            );
        }
        for link in [
            "https://www.youtube.com/channel/UCz75RVbH8q2jdBJ4SnwuZZQ",
            "https://youtube.com/channel/UCz75RVbH8q2jdBJ4SnwuZZQ/videos",
            "youtube.com/channel/UCz75RVbH8q2jdBJ4SnwuZZQ/",
        ] {
            assert_eq!(
                channel_from_link(link),
                Some(Channel {
                    id: "UCz75RVbH8q2jdBJ4SnwuZZQ".to_owned(),
                    name: String::new(),
                    avatar: None,
                }),
                "{link}"
            );
        }
        assert_eq!(
            channel_from_link("https://www.youtube.com/c/LinusTechTips/videos"),
            Some(Channel {
                id: "c/LinusTechTips".to_owned(),
                name: "LinusTechTips".to_owned(),
                avatar: None,
            })
        );
        assert_eq!(
            channel_from_link("https://www.youtube.com/user/LinusTechTips"),
            Some(Channel {
                id: "user/LinusTechTips".to_owned(),
                name: "LinusTechTips".to_owned(),
                avatar: None,
            })
        );
    }

    #[test]
    fn searches_are_not_links() {
        for text in [
            "programming",
            "dQw4w9WgXcQ",
            "rust egui tutorial",
            "https://example.com/watch?v=dQw4w9WgXcQ",
            "https://www.youtube.com/@channel",
            "https://youtu.be/short",
        ] {
            assert_eq!(video_id_from_link(text), None, "{text}");
        }
        for text in [
            "programming",
            "@BlenderStudio",
            "UCz75RVbH8q2jdBJ4SnwuZZQ",
            "https://www.youtube.com/watch?v=dQw4w9WgXcQ",
            "https://youtu.be/dQw4w9WgXcQ",
            "https://example.com/@BlenderStudio",
            "https://www.youtube.com/@",
            "https://www.youtube.com/channel/short",
            "https://www.youtube.com/c/",
            "https://www.youtube.com/user/",
            "https://www.youtube.com/@BlenderStudio/watch/dQw4w9WgXcQ",
        ] {
            assert_eq!(channel_from_link(text), None, "{text}");
        }
    }
}
