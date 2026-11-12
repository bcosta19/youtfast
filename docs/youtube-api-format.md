# YouTube response formats

YouTFast reads YouTube through InnerTube, the internal JSON API that
youtube.com itself calls, implemented in `crates/youtfast-youtube/src/innertube/`.
InnerTube has no public contract: YouTube changes request parameters and
response layouts without notice. This file records every such change that broke
YouTFast, so the next one is recognised quickly and none of them comes back.

Regression tests are named after their entry (`yt<n>_…`) in
[`crates/youtfast-youtube/tests/youtube_api.rs`](../crates/youtfast-youtube/tests/youtube_api.rs)
and in the `crates/youtfast-youtube/src/innertube/` modules.
Parser fixtures cover the current format and any supported older format;
request routing and live latency are not inferred from parser tests.

The entries below are dated records: the older ones name RustyPipe and its
fork because that was the implementation then. Today the same readers and
token builders live in `crates/youtfast-youtube/src/innertube/`.

## How a format change shows up

- **Refused request.** YouTube answers with an error, for example
  `BadRequest("Request contains an invalid argument.")`, and the page shows
  "Could not complete the YouTube request".
- **Silently empty or wrong page.** YouTube accepts the request, but an item no
  longer fits the readers in `crates/youtfast-youtube/src/innertube/`, which
  skip it, so a tab can come back empty ("No playlists"), or a field can take
  another field's value, with no error. Renderers in an item list that no
  reader recognises are logged (`items::UnrecognisedItems`), with a warning
  when the page yielded nothing. A wrong `params` can also make YouTube answer
  with another tab than the one asked for.

Recorded responses keep passing after YouTube changes. Only a live request
shows a new format: `cargo test -p youtfast-youtube -- --ignored` covers the
public endpoints.

## How to investigate

1. Reproduce outside the app with a small test in
   `crates/youtfast-youtube/tests/youtube_api.rs` (or a unit test next to the
   reader): call the function that loads the page, and print the raw response
   of `Client::post_raw` for the same endpoint. Keep raw responses out of the
   repository.
2. Compare with what youtube.com sends for the same page. The response itself
   carries most answers:
   - which tab is `selected`;
   - the `params` of every tab (`tabRenderer.endpoint.browseEndpoint.params`);
   - the continuation tokens of the sort chips (`chipViewModel.tapCommand`);
   - the keys of each list item.
3. Continuation tokens and `params` are base64 protobuf: decode them field by
   field, and compare with what `src/innertube/channel.rs` builds.
4. Fix the reader or the token builder in `src/innertube/`, and add the entry
   below and its regression test in the same PR.

## Known changes

### YT-1: sort token of a channel tab (2026-10-19)

- **Symptom:** Latest / Popular / Oldest on a channel's Videos, Shorts or Live
  tab fails on every channel with "Could not complete the YouTube request".
  YouTube answers `BadRequest("Request contains an invalid argument.")`.
- **Old format (refused):** under the tab's field (15 Videos, 10 Shorts, 14
  Live), the target id as field 2 (`"\n$<uuid>"`) and the order as fields 3/4/5
  beside it.
- **New format:** the tab's field holds one message, `{1: <uuid>, 3: <order>}`,
  under field 8 (Videos, Live) or 7 (Shorts). Orders: Latest/Popular/Oldest =
  4/2/5 (Videos, Shorts), 12/14/13 (Live).
- **Fix:** `order_ctoken` in the fork's `src/client/channel.rs`.
- **Tests:** `yt1_sort_tokens_match_the_chips_youtube_sends`,
  `yt1_sort_tokens_use_the_envelope_not_the_refused_fields`,
  `yt1_each_order_of_each_tab_has_its_own_token`.

### YT-2: Shorts items without `thumbnail` (2026-10-19)

- **Symptom:** a channel's Shorts tab is empty, sorted or not, although the
  response carries the Shorts.
- **Old format:** `shortsLockupViewModel.thumbnail.sources`, a field rustypipe
  required.
- **New format:** no `thumbnail`. The image is in
  `thumbnailViewModel.thumbnailViewModel.image.sources`. Without the required
  field, every item failed to deserialize and was skipped.
- **Fix:** `ShortsLockupViewModel` in
  the fork's `src/client/response/video_item.rs` reads both; `thumbnail`
  is optional.
- **Tests:** `yt2_a_short_with_only_a_thumbnail_view_model_is_kept`,
  `yt2_a_short_with_the_older_thumbnail_is_still_kept`.

### YT-3: `params` of the Playlists tab (2026-10-20)

- **Symptom:** "No playlists" on every channel.
- **Old format:** `params = "EglwbGF5bGlzdHMgAQ=="`. YouTube does not refuse
  it: it answers with the channel's **Home** tab, whose shelves hold no
  playlist grid.
- **New format:** `params = "EglwbGF5bGlzdHPyBgoKCEIGCgIQaCIA"`, the value
  youtube.com's Playlists tab sends. Videos, Live and Search kept theirs.
- **Fix:** `ChannelTab::Playlists` in the fork's `src/client/channel.rs`.
- **Tests:** `yt3_the_playlists_tab_sends_the_params_youtube_uses`,
  `yt3_the_playlists_tab_never_sends_the_params_that_return_home`,
  `yt3_the_other_tabs_keep_the_params_youtube_uses`.

### YT-4: podcasts in the Playlists tab (2026-10-20)

- **Symptom:** some of a channel's playlists are missing (24 of 30 on one
  page).
- **Old format:** every grid item had `contentType:
  LOCKUP_CONTENT_TYPE_PLAYLIST`.
- **New format:** playlists the channel marked as podcasts come as
  `LOCKUP_CONTENT_TYPE_PODCAST`, with a `PL…` id like any playlist. The unknown
  type mapped to nothing.
- **Fix:** `LockupContentType` in
  the fork's `src/client/response/video_item.rs` reads the podcast type
  as a playlist.
- **Test:** `yt4_a_podcast_is_listed_as_a_playlist`.

### YT-5: counts of a channel's page header (2026-10-20)

- **Symptom:** a channel shows its video count as its subscriber count
  ("151 subscribers" for 572K subscribers and 151 videos). A large video count
  reads wrong ("2.2K videos" as 22).
- **Old format:** each metadata part had a single key, `{"text": …}`.
- **New format:** the subscriber part also carries `accessibilityLabel`. The
  externally tagged `MetadataPart` enum rejected a part with two keys, the list
  skipped it, and the video count took its place. Separately, the video count
  was parsed digit by digit.
- **Fix:** `MetadataPart` in the fork's `src/client/response/mod.rs` is
  untagged and told apart by its field; `header_counts` in
  the fork's `src/client/channel.rs` parses abbreviated video counts.
- **Tests:** `yt5_a_subscriber_part_with_an_accessibility_label_is_read`,
  `yt5_an_abbreviated_video_count_is_not_read_digit_by_digit`.

### YT-6: channel avatars on video lockups (2026-10-30)

- **Symptom:** Home's recommendations for the account (For you) show each
  channel's initial in a grey circle instead of its avatar; titles, channels and
  counts are right.
- **Old format:** video cards were `videoRenderer`s with the avatar in
  `channelThumbnailSupportedRenderers` (still the case in search results).
- **New format:** video cards are `lockupViewModel`s; the avatar sits beside the
  title, in `metadata.lockupMetadataViewModel.image.decoratedAvatarViewModel.avatar.avatarViewModel.image.sources`.
  Our own reader of the first Home page never read an avatar, and rustypipe's
  lockup type had no `image` field, so serde dropped it on every later page.
- **Fix:** `from_lockup` and `from_renderer` in
  `crates/youtfast-youtube/src/innertube/home.rs`; `LockupViewModelMetadataInner` and
  `map_lockup` in the fork's `src/client/response/video_item.rs`.
- **Tests:** `yt6_a_video_lockup_keeps_its_channels_avatar`,
  `yt6_a_video_lockup_with_another_image_is_still_read`.

### YT-7: video lockups on a channel's own tabs (2026-11-10)

- **Symptom:** video cards of a channel's Videos tab show only the title: no
  channel, views, age or length. Up next has the same gaps for videos queued
  from such a tab, but not for related videos, so the problem appears and
  disappears there.
- **Old format:** a video lockup's `metadataRows` were `[channel, views ·
  age]`; the length badge was in `thumbnailOverlayBadgeViewModel.thumbnailBadges`.
- **New format:** on the channel's own tabs there is a single row, `[views ·
  age]`, since the channel is the page's. The length badge sits in
  `thumbnailBottomOverlayViewModel.badges`. The lone row was read as the
  channel (and failed, since it has no link), the second row read for views
  was missing, and no overlay matched the old badge shape. Home, search and
  related lists still send the old shape.
- **Fix:** `map_lockup` in the fork's `src/client/response/video_item.rs`
  reads the channel row only when there are two or more rows, and otherwise
  uses the page's channel. `ImageViewOverlay` and `ImageViewOl::badge_text` in
  its `src/client/response/mod.rs` read both badge shapes.
- **Tests:** `yt7_a_lone_metadata_row_is_read_as_views_and_age`,
  `yt7_the_length_in_a_bottom_overlay_is_read`,
  `yt7_a_lockup_with_a_channel_row_keeps_every_field`.

### YT-8: account playlist lockups trigger retries (2026-11-11)

- **Symptom:** Liked videos and Watch later load slowly. Before the fallback
  succeeded, rustypipe retried `InvalidData("itemSectionRenderer empty")`:
  three identical browse requests and 3.4–6 seconds of retry sleeps, then
  the app's reader made a fourth request for the same first page.
- **Old format:** `playlistVideoListRenderer.contents` held
  `playlistVideoRenderer` rows and a `continuationItemRenderer`.
- **New format:** `lockupViewModel` videos sit directly in the page's
  `itemSectionRenderer.contents`; a `continuationItemViewModel` carries the
  next token. rustypipe's playlist mapper fails, and its `InvalidData`
  extraction error is retryable even though the response layout will not change.
- **Fix:** `liked` and `watch_later` in
  `crates/youtfast-youtube/src/innertube.rs` call `playlist::videos` directly,
  bypassing rustypipe's playlist mapper and its extraction retries. The
  first successful authenticated `browse` (`VLLL` / `VLWL`) supplies the
  videos and continuation in one request. `playlist::more` continues with
  the same authentication; the reader also accepts older first-page rows.
  Public playlist details use the same reader.
- **Tests:** `yt8_account_playlist_lockups_keep_their_videos_and_authenticated_continuation`
  and `yt8_account_playlists_still_read_older_first_pages` in
  `crates/youtfast-youtube/src/innertube/playlist.rs` use trimmed synthetic
  pages to cover both layouts and authenticated continuation directions.
  These offline parser tests do not measure request count or latency.
- **Live comparison:** run `youtfast --verbose`, open Liked videos, count
  `WARN dependency` retry lines and time to first rows. The format-triggered
  retry lines should disappear. Repeat for Watch later; do not extract or
  log the account's credential-store session to investigate.

## Adding an entry

Copy this outline. Number entries in order, and never reuse a number.

```markdown
### YT-n: <what changed> (<date found>)

- **Symptom:** what the app showed.
- **Old format:** what YouTube used to send or accept.
- **New format:** what it sends or accepts now, and why the old code failed.
- **Fix:** file and function.
- **Tests:** `ytn_…` in crates/youtfast-youtube/tests/youtube_api.rs.
```
