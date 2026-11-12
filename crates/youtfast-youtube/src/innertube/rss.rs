//! Reading a channel's public Atom RSS feed (`https://www.youtube.com/feeds/videos.xml`).

use super::client::{Client, Error};
use youtfast_core::model::Video;

pub async fn fetch(client: &Client, channel_id: &str) -> Result<Vec<Video>, Error> {
    let url = format!(
        "https://www.youtube.com/feeds/videos.xml?channel_id={}",
        super::client::urlencoded(channel_id)
    );
    match client.get_text(&url).await {
        Ok(xml) => parse(&xml, channel_id),
        Err(err) => {
            if let Ok((_, mut videos, _)) = super::channel::channel_tab(
                client,
                channel_id,
                &youtfast_core::model::ChannelTab::Videos,
                super::ChannelOrder::Latest,
            )
            .await
            {
                videos.truncate(15);
                Ok(videos)
            } else {
                Err(err)
            }
        }
    }
}

pub fn parse(xml: &str, fallback_channel_id: &str) -> Result<Vec<Video>, Error> {
    let doc = roxmltree::Document::parse(xml)
        .map_err(|err| Error::Extraction(format!("invalid channel feed XML: {err}").into()))?;
    let feed = doc.root_element();
    let text_of = |node: roxmltree::Node<'_, '_>, name: &str| {
        node.descendants()
            .find(|n| n.is_element() && n.tag_name().name() == name)
            .and_then(|n| n.text())
            .map(str::trim)
            .map(str::to_owned)
    };
    let feed_channel_id = text_of(feed, "channelId");
    // The feed's own title is a direct child: an entry's title is its video's.
    let feed_channel_name = feed
        .children()
        .find(|n| n.is_element() && n.tag_name().name() == "title")
        .and_then(|n| n.text())
        .map(str::trim)
        .map(str::to_owned);
    let mut videos = Vec::new();
    for entry in feed
        .children()
        .filter(|n| n.is_element() && n.tag_name().name() == "entry")
    {
        let Some(id) = text_of(entry, "videoId") else {
            continue;
        };
        let published_at = text_of(entry, "published")
            .and_then(|iso| iso.parse::<jiff::Timestamp>().ok())
            .map(|ts| ts.as_second());
        let views = entry
            .descendants()
            .find(|n| n.is_element() && n.tag_name().name() == "statistics")
            .and_then(|n| n.attribute("views"))
            .and_then(|v| v.parse::<u64>().ok())
            .filter(|&v| v > 0);
        videos.push(Video {
            id,
            title: text_of(entry, "title").unwrap_or_default(),
            channel: feed_channel_name.clone(),
            channel_avatar: None,
            channel_id: Some(
                feed_channel_id
                    .clone()
                    .unwrap_or_else(|| fallback_channel_id.to_owned()),
            ),
            duration: None,
            views,
            live: false,
            published_at,
            published: None,
            short: false,
        });
    }
    Ok(videos)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_channel_atom_feed() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<feed xmlns:yt="http://www.youtube.com/xml/schemas/2015" xmlns:media="http://search.yahoo.com/mrss/">
  <yt:channelId>UCuAXFkgsw1L7xaCfnd5JJOw</yt:channelId>
  <title>Rick &amp; Co</title>
  <entry>
    <yt:videoId>dQw4w9WgXcQ</yt:videoId>
    <title>Never Gonna &quot;Give&quot; You Up</title>
    <published>2009-10-25T06:57:33+00:00</published>
    <media:group>
      <media:community>
        <media:statistics views="1700000000"/>
      </media:community>
    </media:group>
  </entry>
  <entry>
    <yt:videoId>aaaaaaaaaaa</yt:videoId>
    <title>Upcoming Premiere</title>
    <published>2026-10-25T00:00:00+00:00</published>
    <media:group>
      <media:community>
        <media:statistics views="0"/>
      </media:community>
    </media:group>
  </entry>
</feed>"#;
        let videos = parse(xml, "UCfallback").unwrap();
        assert_eq!(videos.len(), 2);
        assert_eq!(videos[0].id, "dQw4w9WgXcQ");
        assert_eq!(videos[0].title, "Never Gonna \"Give\" You Up");
        assert_eq!(videos[0].channel.as_deref(), Some("Rick & Co"));
        assert_eq!(
            videos[0].channel_id.as_deref(),
            Some("UCuAXFkgsw1L7xaCfnd5JJOw")
        );
        assert_eq!(videos[0].published_at, Some(1_256_453_853));
        assert_eq!(videos[0].views, Some(1_700_000_000));
        assert_eq!(videos[1].views, None);
    }

    #[test]
    fn entities_are_decoded_once_and_numeric_references_count() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<feed xmlns:yt="http://www.youtube.com/xml/schemas/2015">
  <yt:channelId>UCx</yt:channelId>
  <title>Literal &amp;lt; &amp; &#39;quotes&#39;</title>
  <entry>
    <yt:videoId>aaaaaaaaaaa</yt:videoId>
    <title>&#65; &amp;amp; B</title>
  </entry>
</feed>"#;
        let videos = parse(xml, "UCfallback").unwrap();
        assert_eq!(videos[0].title, "A &amp; B");
        assert_eq!(
            videos[0].channel.as_deref(),
            Some("Literal &lt; & 'quotes'")
        );
        assert!(parse("not xml", "UCfallback").is_err());
    }
}
