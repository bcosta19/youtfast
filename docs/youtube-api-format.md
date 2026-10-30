# YouTube response formats

YouTFast reads YouTube through InnerTube, the internal JSON API that
youtube.com itself calls, by way of the rustypipe copy in `vendor/rustypipe`.
InnerTube has no public contract: YouTube changes request parameters and
response layouts without notice. This file records every such change that broke
YouTFast, so the next one is recognised quickly and none of them comes back.

Every entry has a regression test in [`tests/youtube_api.rs`](../tests/youtube_api.rs)
named after it (`yt<n>_…`). The test passes on the format YouTube uses now and
fails if the code goes back to the old one.

## How a format change shows up

- **Refused request.** YouTube answers with an error, for example
  `BadRequest("Request contains an invalid argument.")`, and the page shows
  "Could not complete the YouTube request".
- **Silently empty or wrong page.** YouTube accepts the request, but an item no
  longer fits rustypipe's structs. Lists deserialize through `MapResult`/
  `VecSkipError`, which skip such an item with only a log warning, so a tab can
  come back empty ("No playlists"), or a field can take another field's value,
  with no error. A wrong `params` can also make YouTube answer with another tab
  than the one asked for.

rustypipe's own unit tests use recorded responses and keep passing after
YouTube changes. They also do not run in CI: the vendored crate is not a
workspace member, and its `testfiles/` fixtures are not vendored. Only a live
request shows a new format.

## How to investigate

1. Reproduce with the vendored rustypipe outside the app: a scratch crate that
   depends on `vendor/rustypipe` by path. Call the query (for example
   `channel_playlists`), and also fetch the raw response with
   `RustyPipeQuery::raw(ClientType::Desktop, "browse", &body)`. Keep raw
   responses out of the repository.
2. Compare with what youtube.com sends for the same page. The response itself
   carries most answers:
   - which tab is `selected`;
   - the `params` of every tab (`tabRenderer.endpoint.browseEndpoint.params`);
   - the continuation tokens of the sort chips (`chipViewModel.tapCommand`);
   - the keys of each list item.
3. Continuation tokens and `params` are base64 protobuf: decode them field by
   field, and compare with what `vendor/rustypipe/src/client/channel.rs` builds.
4. Fix the vendored code, then add the entry below and its regression test in
   the same PR. If the test needs a code path the hooks do not reach yet, add
   one to `vendor/rustypipe/src/client/regression.rs`.

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
- **Fix:** `order_ctoken` in `vendor/rustypipe/src/client/channel.rs`.
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
  `vendor/rustypipe/src/client/response/video_item.rs` reads both; `thumbnail`
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
- **Fix:** `ChannelTab::Playlists` in `vendor/rustypipe/src/client/channel.rs`.
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
  `vendor/rustypipe/src/client/response/video_item.rs` reads the podcast type
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
- **Fix:** `MetadataPart` in `vendor/rustypipe/src/client/response/mod.rs` is
  untagged and told apart by its field; `header_counts` in
  `vendor/rustypipe/src/client/channel.rs` parses abbreviated video counts.
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
  `src/youtube/innertube/home.rs`; `LockupViewModelMetadataInner` and
  `map_lockup` in `vendor/rustypipe/src/client/response/video_item.rs`.
- **Tests:** `yt6_a_video_lockup_keeps_its_channels_avatar`,
  `yt6_a_video_lockup_with_another_image_is_still_read`.

## Adding an entry

Copy this outline. Number entries in order, and never reuse a number.

```markdown
### YT-n: <what changed> (<date found>)

- **Symptom:** what the app showed.
- **Old format:** what YouTube used to send or accept.
- **New format:** what it sends or accepts now, and why the old code failed.
- **Fix:** file and function.
- **Tests:** `ytn_…` in tests/youtube_api.rs.
```
