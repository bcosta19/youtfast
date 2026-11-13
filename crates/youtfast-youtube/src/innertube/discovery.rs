//! Search and discovery choices owned by the app.

use serde_json::{Value, json};

use super::More;
use super::client::{
    Client, ContinuationEndpoint, base64_urlencoded, proto_bytes, proto_varint, urlencoded,
};
use super::items::{
    collect_channels_with_meta_and_ctoken, collect_playlists_and_ctoken, collect_videos_and_ctoken,
};
use youtfast_core::model::{ChannelMeta, PageItems};

macro_rules! choices {
    ($name:ident, $default:ident => $label:literal, $($variant:ident => $text:literal),+ $(,)?) => {
        #[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
        pub enum $name {
            #[default]
            $default,
            $($variant),+
        }
        impl $name {
            pub const ALL: &'static [Self] = &[Self::$default, $(Self::$variant),+];
            pub fn label(self) -> &'static str {
                match self { Self::$default => $label, $(Self::$variant => $text),+ }
            }
        }
    };
}

choices!(SearchKind, Videos => "Videos", Channels => "Channels", Playlists => "Playlists");
choices!(SearchOrder, Relevance => "Relevance", Date => "Upload date", Views => "View count", Rating => "Rating");
choices!(SearchDate, Any => "Any time", Hour => "Last hour", Day => "Today", Week => "This week", Month => "This month", Year => "This year");
choices!(SearchLength, Any => "Any duration", Short => "Under 4 minutes", Medium => "4–20 minutes", Long => "Over 20 minutes");
choices!(ChannelOrder, Latest => "Latest", Popular => "Popular", Oldest => "Oldest");

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SearchOptions {
    pub kind: SearchKind,
    pub order: SearchOrder,
    pub date: SearchDate,
    pub length: SearchLength,
    pub live: bool,
}

impl SearchOptions {
    /// Encodes the search filter as YouTube's `params` (`sp`) protobuf.
    /// Video-only choices never leak into a channel or playlist request.
    pub fn params(self) -> Option<String> {
        let mut filter = Vec::new();
        if self.kind == SearchKind::Videos {
            let date = match self.date {
                SearchDate::Any => None,
                SearchDate::Hour => Some(1),
                SearchDate::Day => Some(2),
                SearchDate::Week => Some(3),
                SearchDate::Month => Some(4),
                SearchDate::Year => Some(5),
            };
            if let Some(d) = date {
                proto_varint(1, d, &mut filter);
            }
        }
        let item_type = match self.kind {
            SearchKind::Videos => 1,
            SearchKind::Channels => 2,
            SearchKind::Playlists => 3,
        };
        proto_varint(2, item_type, &mut filter);

        if self.kind == SearchKind::Videos {
            let length = match self.length {
                SearchLength::Any => None,
                SearchLength::Short => Some(1),
                SearchLength::Medium => Some(3),
                SearchLength::Long => Some(2),
            };
            if let Some(l) = length {
                proto_varint(3, l, &mut filter);
            }
            if self.live {
                proto_varint(8, 1, &mut filter);
            }
        }

        let mut root = Vec::new();
        if self.kind == SearchKind::Videos {
            let sort = match self.order {
                SearchOrder::Relevance => None,
                SearchOrder::Date => Some(2),
                SearchOrder::Views => Some(3),
                SearchOrder::Rating => Some(1),
            };
            if let Some(s) = sort {
                proto_varint(1, s, &mut root);
            }
        }
        proto_bytes(2, &filter, &mut root);
        Some(base64_urlencoded(&root))
    }
}

pub async fn search_filtered_with_meta(
    rp: &Client,
    query: &str,
    options: SearchOptions,
) -> Result<(PageItems, Vec<(String, ChannelMeta)>, Option<More>), String> {
    let mut body = json!({ "query": query });
    if let Some(params) = options.params() {
        body["params"] = Value::String(params);
    }
    let page = rp
        .post_raw("search", &body, false, None)
        .await
        .map_err(super::text)?;
    let visitor_data = page
        .pointer("/responseContext/visitorData")
        .and_then(Value::as_str)
        .map(str::to_owned);
    let make_more = |ctoken: Option<String>| {
        ctoken.map(|ctoken| More {
            ctoken,
            endpoint: ContinuationEndpoint::Search,
            visitor_data: visitor_data.clone(),
            authenticated: false,
            ours: false,
            owner: None,
        })
    };
    Ok(match options.kind {
        SearchKind::Videos => {
            let (videos, ctoken) = collect_videos_and_ctoken(&page);
            (PageItems::Videos(videos), Vec::new(), make_more(ctoken))
        }
        SearchKind::Channels => {
            let (channels, metas, ctoken) = collect_channels_with_meta_and_ctoken(&page);
            (PageItems::Channels(channels), metas, make_more(ctoken))
        }
        SearchKind::Playlists => {
            let (playlists, ctoken) = collect_playlists_and_ctoken(&page);
            (
                PageItems::Playlists(playlists),
                Vec::new(),
                make_more(ctoken),
            )
        }
    })
}

pub async fn search_filtered(
    rp: &Client,
    query: &str,
    options: SearchOptions,
) -> Result<(PageItems, Option<More>), String> {
    let (items, _, more) = search_filtered_with_meta(rp, query, options).await?;
    Ok((items, more))
}

/// Fetches search suggestions from YouTube's autocomplete endpoint, signed out.
pub async fn suggestions(rp: &Client, query: &str) -> Result<Vec<String>, String> {
    let url = format!(
        "https://clients1.google.com/complete/search?client=firefox&ds=yt&hl=en&q={}",
        urlencoded(query)
    );
    let text = rp.get_text(&url).await.map_err(super::text)?;
    Ok(parse_suggestions(&text))
}

fn parse_suggestions(raw: &str) -> Vec<String> {
    let trimmed = raw.trim();
    let json_str = trimmed
        .strip_prefix("window.google.ac.h(")
        .and_then(|s| s.strip_suffix(')'))
        .unwrap_or(trimmed);
    let Ok(val) = serde_json::from_str::<Value>(json_str) else {
        return Vec::new();
    };
    let Some(items) = val.get(1).and_then(Value::as_array) else {
        return Vec::new();
    };
    items
        .iter()
        .filter_map(|item| {
            item.as_str()
                .or_else(|| item.get(0).and_then(Value::as_str))
                .map(str::to_owned)
        })
        .collect()
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct PlaylistDetails {
    pub name: String,
    pub thumbnail: Option<String>,
    pub channel: Option<String>,
    pub description: String,
    pub videos: Option<u64>,
}

pub async fn playlist_details(
    rp: &Client,
    id: &str,
) -> Result<
    (
        PlaylistDetails,
        Vec<youtfast_core::model::Video>,
        Option<More>,
    ),
    String,
> {
    super::playlist::page_details(rp, id).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn video_only_filters_do_not_leak_into_channel_or_playlist_params() {
        let base_channel = SearchOptions {
            kind: SearchKind::Channels,
            ..SearchOptions::default()
        };
        let noisy_channel = SearchOptions {
            kind: SearchKind::Channels,
            order: SearchOrder::Views,
            date: SearchDate::Week,
            length: SearchLength::Long,
            live: true,
        };
        assert_eq!(base_channel.params(), noisy_channel.params());
    }

    #[test]
    fn parses_autocomplete_suggestions_in_both_formats() {
        assert_eq!(
            parse_suggestions(r#"["rust",["rust","rust tutorial","rust egui"]]"#),
            vec!["rust", "rust tutorial", "rust egui"]
        );
        assert_eq!(
            parse_suggestions(r#"window.google.ac.h(["rust",[["rust",0],["rust gui",0]]])"#),
            vec!["rust", "rust gui"]
        );
    }
}
