//! Everything that goes out to the network for a video: InnerTube through
//! rustypipe, playback extraction through yt-dlp, SponsorBlock, captions
//! and storyboards, and the thumbnails' disk cache.

pub mod extras;
pub mod innertube;
pub mod lounge;
pub mod sponsorblock;
pub mod thumbs;
pub mod ytdlp;
