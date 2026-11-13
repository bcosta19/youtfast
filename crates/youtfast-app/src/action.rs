//! What the views ask the application to do.

use youtfast_core::model::{Channel, ChannelTab, FeedFilter, Quality, Rating, Source, Video, View};

/// What a view asks the app to do. Views push these while drawing; the app
/// applies them after the frame.
#[derive(Clone, Debug, PartialEq)]
pub enum Action {
    Cast(youtfast_cast::Action),
    SetSearchOptions(youtfast_youtube::innertube::SearchOptions),
    SetChannelOrder(youtfast_youtube::innertube::ChannelOrder),
    SetHistoryQuery(String),
    SetWatchLaterQuery(String),
    SetLikedQuery(String),
    SetPlaylistsQuery(String),
    RemoveHistory(String),
    PlayPlaylist,
    /// A search, or a YouTube link to open.
    Search(String),
    Open(Video),
    OpenVideoTab(Video),
    SelectVideoTab(String),
    CloseVideoTab(String),
    /// The "+" button: go Home and open the next video in a new tab.
    NewVideoTab,
    /// Goes back to where the current page was opened from.
    Back,
    Forward,
    Navigate(View),
    OpenChannel(Channel),
    OpenPlaylist {
        id: String,
        name: String,
    },
    /// Loads the page on screen again.
    Refresh,
    SetSource(Source),
    SetHistoryFilter(youtfast_logic::history::HistoryFilter),
    SetSubscriptionsQuery(String),
    SetSubscriptionsSort(youtfast_core::model::SubscriptionSort),
    TogglePause,
    /// The desktop's media controls ask for play or pause directly.
    SetPaused(bool),
    /// Seconds from the start; `exact` once the seek bar is released.
    SeekTo {
        seconds: f64,
        exact: bool,
    },
    SeekBy(f64),
    /// 0 to 100.
    SetVolume(f64),
    ToggleFullscreen,
    ExitFullscreen,
    /// The watch page between its usual layout and theater mode.
    ToggleTheater,
    ToggleMiniPlayer,
    SetScrollMiniplayer(bool),
    /// The sidebar's buttons: their order and which are shown.
    SetSidebar(Vec<youtfast_storage::settings::SidebarEntry>),
    SetPlaybackBuffer(youtfast_storage::settings::PlaybackBuffer),
    SetHistorySync(youtfast_storage::settings::HistorySync),
    SeekChapter(bool),
    RetryPlayback,
    ToggleLoop,
    MoveQueue {
        from: usize,
        to: usize,
    },
    SetCaptionScale(f64),
    SetCaptionBackground(bool),
    LoadExtras,
    ToggleTranscript,
    ToggleAudioOnly,
    SetQuality(Quality),
    SetTheme(youtfast_storage::settings::ThemeChoice),
    SetLanguage(youtfast_i18n::Locale),
    /// Draw the interface with this face, now.
    SetInterfaceFont(youtfast_appearance::theme::InterfaceFont),
    /// Open a prefilled GitHub issue about the system font, in the browser.
    ReportInterfaceFont,
    /// Subscribes on this device, or unsubscribes.
    ToggleSubscription(Channel),
    /// Adds to this device's Watch later, or removes.
    ToggleWatchLater(Video),
    ClearHistory,
    ImportTakeout,
    /// "sign in with cookies.txt" for the account being added.
    AddAccountFile,
    /// `sign in with <browser>` for the account being added, with the
    /// browser and profile typed under Advanced.
    AddAccountBrowser,
    /// A detected browser profile's `Sign in`, by its index in
    /// `State::browser_profiles`.
    AddAccountProfile(usize),
    /// The form's Advanced part opened or closed.
    ToggleAccountAdvanced,
    /// The account name/profile fields of the add form (view-owned).
    SetAccountName(String),
    SetAccountProfile(String),
    /// `[ switch ]` on an account row.
    SwitchAccount(u32),
    /// `[ rename ]` editing the name, then Enter saves.
    RenameAccount {
        id: u32,
        name: String,
    },
    /// Update the inline name without saving it yet.
    SetRenameDraft {
        id: u32,
        name: String,
    },
    /// `[ remove ]` after the inline confirm.
    RemoveAccount(u32),
    /// Open an account row's inline rename field / remove confirm.
    MarkRenaming {
        id: u32,
        name: String,
    },
    MarkRemoving(u32),
    /// Dismiss the remove confirm.
    CancelRemoving,
    /// Open the notifications popup and mark its current alerts as visited.
    VisitNotifications,
    /// `[ check now ]` on the Notifications page.
    CheckAlertsNow,
    /// `[ mark all seen ]` on the Notifications page.
    MarkAlertsSeen,
    /// The notifications settings pane's toggles.
    SetAlerts(bool),
    SetDesktopNotifications(bool),
    SetAlertsEvery(u32),
    /// `[ sign out ]`: the session stays kept for switching back.
    SignOut,
    SetBrowser(String),
    DismissNotice,
    /// The next page of comments.
    LoadMoreComments,
    /// Shows a comment's replies, loading them the first time; a second
    /// time hides them.
    ToggleReplies(String),
    /// The description's `[ more ]`/`[ less ]`.
    ToggleDescription,
    /// A long comment's `[ read more ]`/`[ show less ]`, by its id.
    ToggleCommentText(String),
    /// Rate the video being watched on the account.
    Rate(Rating),
    /// The save menu opened: ask for the account's playlists.
    LoadSaveTargets,
    /// Add the video being watched to an account playlist, or take it out.
    SetSaved {
        playlist: String,
        saved: bool,
    },
    /// The share menu's `Start at` box.
    SetShareAtTime(bool),
    /// The sidebar between open and its collapsed rail.
    ToggleSidebar,
    /// Home's chips.
    SetFeedFilter(FeedFilter),
    /// The watch page's `[ share ]`: the link to the clipboard.
    CopyLink(String),
    /// A link in a description or comment: opens in the browser. Only
    /// http/https are honoured.
    OpenUrl(String),
    /// Switch the channel page's tab.
    SetChannelTab(ChannelTab),
    /// The channel page's search field's text (a view-owned field).
    SetChannelQuery(String),
    /// The next page of the current channel tab's list.
    LoadMore,
    /// 0.25 to 2.
    SetSpeed(f64),
    ToggleMute,
    /// One frame ahead (`true`) or back, paused.
    FrameStep(bool),
    /// A part of the video from its start (0 to 1): the number keys.
    SeekFraction(f64),
    /// Shows the captions track (mpv's id) and keeps its language for the
    /// next videos; `None` turns captions off.
    SetSubtitle(Option<i64>),
    /// `c`: captions on in the language kept (or the first offered), or off.
    ToggleCaptions,
    /// A live stream back to its live edge.
    GoLive,
    /// The queue's next video, else Up next's, else the first related.
    PlayNext,
    /// The video before the one playing in Up next (a Short's ↑).
    PlayPrevious,
    /// Stops the countdown to the next video.
    CancelAutoplay,
    SetAutoplay(bool),
    /// Adds a video to the queue: at its end, or first (`next`).
    Enqueue {
        video: Video,
        next: bool,
    },
    /// Takes a video out of the queue, by its id.
    Dequeue(String),
    ClearQueue,
    SetSponsorBlock(bool),
    /// Jumps over the SponsorBlock segment being played.
    SkipSegment,
    /// The mini player's video, back on its watch page.
    ReturnToWatch,
    /// The mini player's close: pauses and hides it.
    CloseMiniPlayer,
    /// Check GitHub Releases for a newer version of YouTFast.
    CheckForUpdates,
    /// Download, verify and install the available update.
    InstallUpdate,
    /// Toggle desktop notifications when a new version is available.
    SetUpdateNotifications(bool),
    /// Toggle automatic installation of minor and patch updates.
    SetAutoUpdate(bool),
    /// Show or hide the full bundled changelog in the About section.
    ToggleFullChangelog,
}
