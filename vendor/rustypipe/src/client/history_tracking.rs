//! YouTFast's opt-in history report transport. Cookies never leave RustyPipe.

use super::{ClientType, Duration, Error, RustyPipe, RustyPipeQuery, header};

fn failed() -> Error {
    Error::Other("history report failed".into())
}

impl RustyPipe {
    /// Whether a playback report can safely receive the signed-in session.
    /// No credentials, nonstandard ports, fragments or redirects are allowed.
    pub fn history_tracking_url_allowed(value: &str) -> bool {
        let Ok(url) = url::Url::parse(value) else {
            return false;
        };
        url.scheme() == "https"
            && url.username().is_empty()
            && url.password().is_none()
            && url.port_or_known_default() == Some(443)
            && url.fragment().is_none()
            && url.host_str().is_some_and(|host| {
                host == "youtube.com" || host.ends_with(".youtube.com")
            })
    }

    /// GET a validated playback-tracking URL with this client's session.
    /// The shared client never follows redirects (`build_with_client`) and
    /// checks certificates as the caller configured it; response bodies and
    /// raw dependency errors are never retained/reported.
    pub async fn history_tracking_get(&self, url: &str) -> Result<(), Error> {
        if !Self::history_tracking_url_allowed(url) {
            return Err(failed());
        }
        let auth = self.user_auth_cookie().map_err(|_| failed())?;
        let mut request = self
            .inner
            .http
            .get(url)
            .timeout(Duration::from_secs(20))
            .header(
                header::USER_AGENT,
                self.query().user_agent(ClientType::Desktop).as_ref(),
            )
            .header(header::COOKIE, auth.cookie.as_str())
            .header(header::ORIGIN, "https://www.youtube.com")
            .header(header::REFERER, "https://www.youtube.com/");
        if let Some(header) = RustyPipeQuery::sapisidhash_header(&auth.cookie, ClientType::Desktop) {
            request = request.header(header::AUTHORIZATION, header);
        }
        if let Some(index) = auth.session_index {
            request = request.header("X-Goog-AuthUser", index);
        }
        if let Some(channel) = auth.channel_syncid {
            request = request.header("X-Goog-PageId", channel);
        }
        let response = request.send().await.map_err(|_| failed())?;
        if response.status().is_success() {
            Ok(())
        } else {
            Err(failed())
        }
    }
}
