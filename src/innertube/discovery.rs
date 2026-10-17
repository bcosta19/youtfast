//! Search and discovery choices owned by the app, independent of RustyPipe.

use rustypipe::client::RustyPipe;
use rustypipe::model::{ChannelItem, PlaylistItem, VideoItem, richtext::ToPlaintext};
use rustypipe::param::search_filter::{Feature, ItemType, Length, Order, SearchFilter, UploadDate};

use super::{More, channel, largest, playlist, text, video};
use crate::backend::PageItems;

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

choices!(SearchKind, Videos => "Vídeos", Channels => "Canais", Playlists => "Playlists");
choices!(SearchOrder, Relevance => "Relevância", Date => "Data de envio", Views => "Visualizações", Rating => "Avaliação");
choices!(SearchDate, Any => "Qualquer data", Hour => "Última hora", Day => "Hoje", Week => "Esta semana", Month => "Este mês", Year => "Este ano");
choices!(SearchLength, Any => "Qualquer duração", Short => "Menos de 4 minutos", Medium => "De 4 a 20 minutos", Long => "Mais de 20 minutos");
choices!(ChannelOrder, Latest => "Mais recentes", Popular => "Populares", Oldest => "Mais antigos");

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SearchOptions {
    pub kind: SearchKind,
    pub order: SearchOrder,
    pub date: SearchDate,
    pub length: SearchLength,
    pub live: bool,
}

impl SearchOptions {
    /// Video-only choices never leak into a channel or playlist request.
    fn filter(self) -> SearchFilter {
        let filter = SearchFilter::new().item_type(match self.kind {
            SearchKind::Videos => ItemType::Video,
            SearchKind::Channels => ItemType::Channel,
            SearchKind::Playlists => ItemType::Playlist,
        });
        if self.kind != SearchKind::Videos {
            return filter;
        }
        let filter = filter
            .sort_opt(match self.order {
                SearchOrder::Relevance => None,
                SearchOrder::Date => Some(Order::Date),
                SearchOrder::Views => Some(Order::Views),
                SearchOrder::Rating => Some(Order::Rating),
            })
            .date_opt(match self.date {
                SearchDate::Any => None,
                SearchDate::Hour => Some(UploadDate::Hour),
                SearchDate::Day => Some(UploadDate::Day),
                SearchDate::Week => Some(UploadDate::Week),
                SearchDate::Month => Some(UploadDate::Month),
                SearchDate::Year => Some(UploadDate::Year),
            })
            .length_opt(match self.length {
                SearchLength::Any => None,
                SearchLength::Short => Some(Length::Short),
                SearchLength::Medium => Some(Length::Medium),
                SearchLength::Long => Some(Length::Long),
            });
        if self.live {
            filter.feature(Feature::IsLive)
        } else {
            filter
        }
    }
}

pub async fn search_filtered(
    rp: &RustyPipe,
    query: &str,
    options: SearchOptions,
) -> Result<(PageItems, Option<More>), String> {
    let filter = options.filter();
    let request = rp.query();
    Ok(match options.kind {
        SearchKind::Videos => {
            let page = request
                .search_filter::<VideoItem, _>(query, &filter)
                .await
                .map_err(text)?;
            let more = More::of(&page.items);
            (
                PageItems::Videos(page.items.items.into_iter().map(video).collect()),
                more,
            )
        }
        SearchKind::Channels => {
            let page = request
                .search_filter::<ChannelItem, _>(query, &filter)
                .await
                .map_err(text)?;
            let more = More::of(&page.items);
            (
                PageItems::Channels(page.items.items.into_iter().map(channel).collect()),
                more,
            )
        }
        SearchKind::Playlists => {
            let page = request
                .search_filter::<PlaylistItem, _>(query, &filter)
                .await
                .map_err(text)?;
            let more = More::of(&page.items);
            (
                PageItems::Playlists(page.items.items.into_iter().map(playlist).collect()),
                more,
            )
        }
    })
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
    rp: &RustyPipe,
    id: &str,
) -> Result<(PlaylistDetails, Vec<crate::model::Video>, Option<More>), String> {
    match rp.query().playlist(id).await {
        Ok(page) => {
            let details = PlaylistDetails {
                name: page.name.clone(),
                thumbnail: largest(&page.thumbnail),
                channel: page.channel.as_ref().map(|channel| channel.name.clone()),
                description: page
                    .description
                    .as_ref()
                    .map(ToPlaintext::to_plaintext)
                    .unwrap_or_default(),
                videos: Some(page.video_count),
            };
            let (videos, more) = super::playlist_page(page);
            Ok((details, videos, more))
        }
        Err(error) if super::newer_layout(&error) => super::playlist::page_details(rp, id).await,
        Err(error) => Err(text(error)),
    }
}

impl From<ChannelOrder> for rustypipe::param::ChannelOrder {
    fn from(value: ChannelOrder) -> Self {
        match value {
            ChannelOrder::Latest => Self::Latest,
            ChannelOrder::Popular => Self::Popular,
            ChannelOrder::Oldest => Self::Oldest,
        }
    }
}
