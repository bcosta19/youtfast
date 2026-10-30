//! Session-only video tabs. Background tabs never start playback or record history.
//! Where each video was left stays in the library's history, whose resume rule
//! (restart near the start or end) the tabs share.
use crate::model::Video;

#[derive(Debug)]
struct Tab<C> {
    video: Video,
    /// What the tab was playing from (its list, playlist) while another tab
    /// played; given back when it is selected again.
    context: Option<C>,
}

#[derive(Debug)]
pub struct VideoTabs<C = ()> {
    tabs: Vec<Tab<C>>,
    active: Option<String>,
    /// Set by the "+" button: the next new video opens its own tab. Cleared by
    /// `open` (new or existing video) and when the last tab closes; selecting a
    /// tab goes through `open`, so it clears too. Background `add` and closing
    /// other tabs leave it alone.
    new_tab_pending: bool,
}

impl<C> Default for VideoTabs<C> {
    fn default() -> Self {
        Self {
            tabs: Vec::new(),
            active: None,
            new_tab_pending: false,
        }
    }
}

impl<C> VideoTabs<C> {
    pub fn videos(&self) -> impl Iterator<Item = &Video> {
        self.tabs.iter().map(|tab| &tab.video)
    }

    pub fn active(&self) -> Option<&str> {
        self.active.as_deref()
    }

    pub fn video(&self, id: &str) -> Option<&Video> {
        self.videos().find(|video| video.id == id)
    }

    pub fn selected(&self) -> Option<&Video> {
        self.video(self.active()?)
    }

    pub fn add(&mut self, video: Video) {
        if self.video(&video.id).is_none() {
            self.tabs.push(Tab {
                video,
                context: None,
            });
        }
    }

    #[cfg(test)]
    pub fn new_tab_pending(&self) -> bool {
        self.new_tab_pending
    }

    /// The next ordinary open adds a tab instead of replacing the selected one.
    pub fn request_new_tab(&mut self) {
        self.new_tab_pending = true;
    }

    /// Ordinary navigation replaces the selected tab, or adds one after
    /// `request_new_tab`; an existing tab is selected.
    pub fn open(&mut self, video: Video) {
        let new_tab = std::mem::take(&mut self.new_tab_pending);
        if self.video(&video.id).is_none() {
            if new_tab {
                self.add(video.clone());
            } else if let Some(tab) = self
                .tabs
                .iter_mut()
                .find(|tab| Some(&tab.video.id) == self.active.as_ref())
            {
                *tab = Tab {
                    video: video.clone(),
                    context: None,
                };
            } else {
                self.add(video.clone());
            }
        }
        self.active = Some(video.id);
    }

    pub fn update(&mut self, video: &Video) {
        if let Some(tab) = self.tabs.iter_mut().find(|tab| tab.video.id == video.id) {
            tab.video = video.clone();
        }
    }

    /// Keeps what `id` was playing from while another tab plays.
    pub fn stash(&mut self, id: &str, context: C) {
        if let Some(tab) = self.tabs.iter_mut().find(|tab| tab.video.id == id) {
            tab.context = Some(context);
        }
    }

    pub fn take_context(&mut self, id: &str) -> Option<C> {
        self.tabs
            .iter_mut()
            .find(|tab| tab.video.id == id)?
            .context
            .take()
    }

    pub fn close(&mut self, id: &str) {
        let Some(index) = self.tabs.iter().position(|tab| tab.video.id == id) else {
            return;
        };
        self.tabs.remove(index);
        if self.tabs.is_empty() {
            self.new_tab_pending = false;
        }
        if self.active() == Some(id) {
            self.active = self
                .tabs
                .get(index.min(self.tabs.len().saturating_sub(1)))
                .map(|tab| tab.video.id.clone());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn background_open_preserves_selection_and_switch_returns_contexts() {
        let mut tabs = VideoTabs::<&str>::default();
        tabs.open(Video::from_id("first".into()));
        tabs.add(Video::from_id("second".into()));
        tabs.add(Video::from_id("second".into()));
        assert_eq!(tabs.active(), Some("first"));
        assert_eq!(tabs.videos().count(), 2);
        tabs.stash("first", "playlist");
        tabs.open(Video::from_id("second".into()));
        assert_eq!(tabs.take_context("second"), None);
        tabs.open(Video::from_id("first".into()));
        assert_eq!(tabs.take_context("first"), Some("playlist"));
        assert_eq!(tabs.take_context("first"), None);
        // Replacing the selected tab drops what it was playing from.
        tabs.stash("first", "playlist");
        tabs.open(Video::from_id("third".into()));
        assert!(tabs.video("first").is_none());
        assert_eq!(tabs.take_context("third"), None);
        assert_eq!(tabs.videos().count(), 2);
    }

    #[test]
    fn open_after_new_tab_adds_and_selects_instead_of_replacing() {
        let mut tabs = VideoTabs::<()>::default();
        tabs.open(Video::from_id("a".into()));
        tabs.request_new_tab();
        assert!(tabs.new_tab_pending());
        tabs.open(Video::from_id("b".into()));
        assert!(!tabs.new_tab_pending());
        assert_eq!(tabs.active(), Some("b"));
        assert_eq!(tabs.videos().count(), 2);
        tabs.open(Video::from_id("c".into()));
        assert!(tabs.video("b").is_none());
        assert_eq!(tabs.videos().count(), 2);
    }

    #[test]
    fn opening_an_existing_tab_clears_pending_new_tab() {
        let mut tabs = VideoTabs::<()>::default();
        tabs.open(Video::from_id("a".into()));
        tabs.add(Video::from_id("b".into()));
        tabs.request_new_tab();
        tabs.add(Video::from_id("c".into()));
        assert!(tabs.new_tab_pending());
        tabs.open(Video::from_id("b".into()));
        assert!(!tabs.new_tab_pending());
        assert_eq!(tabs.active(), Some("b"));
        assert_eq!(tabs.videos().count(), 3);
        tabs.open(Video::from_id("d".into()));
        assert!(tabs.video("b").is_none());
        assert_eq!(tabs.videos().count(), 3);
    }

    #[test]
    fn closing_the_last_tab_clears_pending_new_tab() {
        let mut tabs = VideoTabs::<()>::default();
        tabs.open(Video::from_id("a".into()));
        tabs.request_new_tab();
        tabs.close("a");
        assert!(!tabs.new_tab_pending());
    }

    #[test]
    fn closing_selects_neighbor_and_background_close_preserves_selection() {
        let mut tabs = VideoTabs::<()>::default();
        tabs.open(Video::from_id("a".into()));
        tabs.add(Video::from_id("b".into()));
        tabs.add(Video::from_id("c".into()));
        tabs.close("b");
        assert_eq!(tabs.active(), Some("a"));
        tabs.close("a");
        assert_eq!(tabs.active(), Some("c"));
        tabs.close("c");
        tabs.close("missing");
        assert_eq!(tabs.active(), None);
        assert_eq!(tabs.videos().count(), 0);
    }
}
