//! The vocabulary every YouTFast crate shares: videos, channels, listings
//! and the other types of [`model`], formatting helpers ([`util`]), YouTube
//! link parsing ([`links`]) and the fixed messages external errors become
//! ([`privacy`]). Nothing here performs IO or draws.

pub mod links;
pub mod model;
pub mod privacy;
pub mod util;
