//! Every icon the interface draws: shared Lucide icons from
//! fastframe-icons, and YouTFast's own Lucide files in `assets/icons/`
//! (theater mode's).

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
        Theater => "rectangle-horizontal",
        Check => lucide "check",
    }
}
