//! Every icon the interface draws. They are all shared Lucide icons from
//! fastframe-icons; YouTFast's own files would join them in
//! `assets/icons/`.

fastframe_icons::icons! {
    /// Every icon the interface draws.
    pub enum Icon {
        prefix: "youtfast-icon-",
        directory: "../assets/icons/",
        Back => lucide "arrow-left",
        Play => lucide "play",
        Pause => lucide "pause",
        Volume => lucide "volume-2",
        Fullscreen => lucide "maximize-2",
        ExitFullscreen => lucide "minimize-2",
        Check => lucide "check",
    }
}
