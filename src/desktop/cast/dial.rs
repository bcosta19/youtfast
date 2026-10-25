//! Local DIAL discovery and app lifecycle. Discovery only performs reads.
//! Samsung also advertises AirPlay when SSDP is filtered; that is a discovery
//! hint, never permission to send media through AirPlay or to launch an app.

use std::collections::HashSet;
use std::net::IpAddr;
use std::time::Duration;

use mdns_sd::{ServiceDaemon, ServiceEvent};
use quick_xml::events::Event;
use reqwest::{Client, Method};
use url::Url;

use super::{Device, Error, Protocol, local_address};

const ORIGIN: &str = "https://www.youtube.com";
const LIMIT: usize = 64 * 1024;

pub(super) fn valid_endpoint(endpoint: &Url, host: IpAddr) -> bool {
    local_address(host)
        && endpoint.scheme() == "http"
        && endpoint
            .host_str()
            .and_then(|host| host.trim_matches(['[', ']']).parse::<IpAddr>().ok())
            == Some(host)
        && endpoint.username().is_empty()
        && endpoint.password().is_none()
        && endpoint.query().is_none()
        && endpoint.fragment().is_none()
        && endpoint.path().ends_with("/YouTube")
}

fn client() -> Result<Client, Error> {
    crate::youtube::innertube::http()
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .connect_timeout(Duration::from_secs(2))
        .timeout(Duration::from_secs(4))
        .build()
        .map_err(|_| Error::Unavailable)
}

async fn body(mut response: reqwest::Response) -> Result<Vec<u8>, Error> {
    let mut bytes = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(|_| Error::Failed)? {
        if bytes.len() + chunk.len() > LIMIT {
            return Err(Error::InvalidResponse);
        }
        bytes.extend_from_slice(&chunk);
    }
    Ok(bytes)
}

struct AppInfo {
    state: String,
    screen: String,
    stop: Option<String>,
}

fn app_info(bytes: &[u8]) -> Result<AppInfo, Error> {
    let mut reader = quick_xml::Reader::from_reader(bytes);
    reader.config_mut().trim_text(true);
    let mut tag = Vec::new();
    let mut name = String::new();
    let mut info = AppInfo {
        state: String::new(),
        screen: String::new(),
        stop: None,
    };
    loop {
        match reader.read_event().map_err(|_| Error::InvalidResponse)? {
            Event::Start(element) | Event::Empty(element) => {
                tag = element.local_name().as_ref().to_vec();
                if tag == b"link" {
                    for attribute in element.attributes() {
                        let attribute = attribute.map_err(|_| Error::InvalidResponse)?;
                        if attribute.key.as_ref() == b"href" {
                            info.stop = Some(
                                attribute
                                    .normalized_value(quick_xml::XmlVersion::Implicit1_0)
                                    .map_err(|_| Error::InvalidResponse)?
                                    .into_owned(),
                            );
                        }
                    }
                }
            }
            Event::Text(text) => {
                let decoded = text.decode().map_err(|_| Error::InvalidResponse)?;
                let text =
                    quick_xml::escape::unescape(&decoded).map_err(|_| Error::InvalidResponse)?;
                match tag.as_slice() {
                    b"name" => name = text.into_owned(),
                    b"state" => info.state = text.into_owned(),
                    b"screenId" => info.screen = text.into_owned(),
                    _ => {}
                }
            }
            Event::End(_) => tag.clear(),
            Event::Eof => break,
            _ => {}
        }
    }
    if name != "YouTube"
        || !matches!(info.state.as_str(), "running" | "stopped" | "hidden")
        || info.screen.len() > 1024
    {
        return Err(Error::NoYouTube);
    }
    Ok(info)
}

async fn info(client: &Client, endpoint: &Url, host: IpAddr) -> Result<AppInfo, Error> {
    if !valid_endpoint(endpoint, host) {
        return Err(Error::InvalidRequest);
    }
    let response = client
        .get(endpoint.clone())
        .header("Origin", ORIGIN)
        .send()
        .await
        .map_err(|_| Error::NoYouTube)?;
    if !response.status().is_success() {
        return Err(Error::NoYouTube);
    }
    app_info(&body(response).await?)
}

pub(super) async fn find(host: IpAddr) -> Result<Device, Error> {
    if !local_address(host) {
        return Err(Error::InvalidRequest);
    }
    let client = client()?;
    for (port, path) in [(8080, "/ws/apps/YouTube"), (8008, "/apps/YouTube")] {
        let endpoint = endpoint(host, port, path)?;
        if info(&client, &endpoint, host).await.is_ok() {
            return Ok(Device {
                host,
                name: "YouTube on TV".into(),
                protocol: Protocol::YouTube { endpoint },
            });
        }
    }
    Err(Error::NoYouTube)
}

fn endpoint(host: IpAddr, port: u16, path: &str) -> Result<Url, Error> {
    let mut url = Url::parse("http://localhost").map_err(|_| Error::InvalidRequest)?;
    url.set_ip_host(host).map_err(|_| Error::InvalidRequest)?;
    url.set_port(Some(port))
        .map_err(|_| Error::InvalidRequest)?;
    url.set_path(path);
    Ok(url)
}

pub(super) async fn launch(endpoint: &Url, host: IpAddr) -> Result<String, Error> {
    let client = client()?;
    let app = info(&client, endpoint, host).await?;
    // A previous remote can leave a queued video behind the TV's login screen.
    // Start a fresh receiver before creating our Lounge session.
    if app.state != "stopped" {
        stop(endpoint, host).await?;
        let mut stopped = false;
        for _ in 0..10 {
            if info(&client, endpoint, host).await?.state == "stopped" {
                stopped = true;
                break;
            }
            tokio::time::sleep(Duration::from_millis(500)).await;
        }
        if !stopped {
            return Err(Error::Unconfirmed);
        }
    }
    let response = client
        .post(endpoint.clone())
        .header("Origin", ORIGIN)
        .header("Content-Type", "text/plain; charset=utf-8")
        // Samsung rejects an empty launch without an explicit length (411).
        .header("Content-Length", "0")
        .body("")
        .send()
        .await
        .map_err(|_| Error::Failed)?;
    #[cfg(test)]
    eprintln!("DIAL launch status: {}", response.status().as_u16());
    if !response.status().is_success() {
        return Err(Error::Failed);
    }
    // Starting a TV app can take considerably longer than a Cast receiver.
    for _ in 0..30 {
        if let Ok(app) = info(&client, endpoint, host).await
            && !app.screen.is_empty()
        {
            return Ok(app.screen);
        }
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
    Err(Error::Unconfirmed)
}

pub(super) async fn stop(endpoint: &Url, host: IpAddr) -> Result<(), Error> {
    let client = client()?;
    let app = info(&client, endpoint, host).await?;
    if app.state == "stopped" {
        return Ok(());
    }
    let base = Url::parse(&format!("{}/", endpoint.as_str())).map_err(|_| Error::InvalidRequest)?;
    let target = base
        .join(app.stop.as_deref().ok_or(Error::Failed)?)
        .map_err(|_| Error::InvalidRequest)?;
    if target.origin() != endpoint.origin()
        || !target.path().starts_with(base.path())
        || target.query().is_some()
        || target.fragment().is_some()
        || !target.username().is_empty()
        || target.password().is_some()
    {
        return Err(Error::InvalidResponse);
    }
    let response = client
        .request(Method::DELETE, target)
        .header("Origin", ORIGIN)
        .header("Content-Length", "0")
        .send()
        .await
        .map_err(|_| Error::Failed)?;
    if !response.status().is_success() {
        return Err(Error::Failed);
    }
    Ok(())
}

struct Mdns(ServiceDaemon);
impl Drop for Mdns {
    fn drop(&mut self) {
        let _ = self.0.shutdown();
    }
}

pub(super) async fn discover() -> Result<Vec<Device>, Error> {
    let (airplay, descriptions) = tokio::join!(airplay_hosts(), ssdp_descriptions());
    let client = client()?;
    let mut found = Vec::new();
    let mut tasks = tokio::task::JoinSet::new();
    for (host, name) in airplay.unwrap_or_default().into_iter().take(32) {
        tasks.spawn(async move {
            find(host).await.map(|mut device| {
                device.name = name;
                device
            })
        });
    }
    for (host, location) in descriptions.unwrap_or_default().into_iter().take(32) {
        let client = client.clone();
        tasks.spawn(async move {
            let response = client
                .get(location)
                .send()
                .await
                .map_err(|_| Error::Failed)?;
            let base = response
                .headers()
                .get("Application-URL")
                .and_then(|h| h.to_str().ok())
                .ok_or(Error::NoYouTube)?;
            let endpoint = Url::parse(&format!("{}/YouTube", base.trim_end_matches('/')))
                .map_err(|_| Error::InvalidResponse)?;
            info(&client, &endpoint, host).await?;
            Ok(Device {
                host,
                name: "YouTube on TV".into(),
                protocol: Protocol::YouTube { endpoint },
            })
        });
    }
    while let Some(Ok(result)) = tasks.join_next().await {
        if let Ok(device) = result
            && !found.iter().any(|known: &Device| known.host == device.host)
        {
            found.push(device);
        }
    }
    Ok(found)
}

async fn airplay_hosts() -> Result<Vec<(IpAddr, String)>, Error> {
    let daemon = Mdns(ServiceDaemon::new().map_err(|_| Error::Unavailable)?);
    let receiver = daemon
        .0
        .browse("_airplay._tcp.local.")
        .map_err(|_| Error::Unavailable)?;
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    let mut hosts = Vec::new();
    while let Ok(Ok(event)) = tokio::time::timeout_at(deadline, receiver.recv_async()).await {
        if let ServiceEvent::ServiceResolved(info) = event {
            let model = info
                .get_property_val_str("model")
                .unwrap_or("YouTube on TV");
            let name: String = model.chars().filter(|c| !c.is_control()).take(80).collect();
            for host in info.get_addresses_v4() {
                let host = IpAddr::V4(host);
                if local_address(host)
                    && !hosts.iter().any(|(ip, _)| *ip == host)
                    && hosts.len() < 32
                {
                    hosts.push((host, name.clone()));
                }
            }
        }
    }
    Ok(hosts)
}

async fn ssdp_descriptions() -> Result<Vec<(IpAddr, Url)>, Error> {
    let socket = tokio::net::UdpSocket::bind("0.0.0.0:0")
        .await
        .map_err(|_| Error::Unavailable)?;
    let message = b"M-SEARCH * HTTP/1.1\r\nHOST: 239.255.255.250:1900\r\nMAN: \"ssdp:discover\"\r\nMX: 2\r\nST: urn:dial-multiscreen-org:service:dial:1\r\n\r\n";
    socket
        .send_to(message, "239.255.255.250:1900")
        .await
        .map_err(|_| Error::Failed)?;
    let deadline = tokio::time::Instant::now() + Duration::from_secs(4);
    let mut buffer = [0; 16 * 1024];
    let mut seen = HashSet::new();
    let mut urls = Vec::new();
    while let Ok(Ok((length, source))) =
        tokio::time::timeout_at(deadline, socket.recv_from(&mut buffer)).await
    {
        if !local_address(source.ip()) || urls.len() >= 32 {
            continue;
        }
        let text = String::from_utf8_lossy(&buffer[..length]);
        for line in text.lines() {
            if let Some((key, value)) = line.split_once(':')
                && key.eq_ignore_ascii_case("location")
                && let Ok(location) = Url::parse(value.trim())
                && location.scheme() == "http"
                && location.host_str().and_then(|h| h.parse::<IpAddr>().ok()) == Some(source.ip())
                && location.username().is_empty()
                && location.password().is_none()
                && location.query().is_none()
                && location.fragment().is_none()
                && seen.insert(location.clone())
            {
                urls.push((source.ip(), location));
            }
        }
    }
    Ok(urls)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dial_status_accepts_namespaces_and_never_treats_an_uninstalled_app_as_a_target() {
        let app = app_info(br#"<service xmlns="urn:dial-multiscreen-org:schemas:dial"><name>YouTube</name><state>running</state><link rel="run" href="run"/><additionalData><screenId>synthetic-screen</screenId></additionalData></service>"#).unwrap();
        assert_eq!(app.state, "running");
        assert_eq!(app.screen, "synthetic-screen");
        assert_eq!(app.stop.as_deref(), Some("run"));
        assert!(app_info(b"<service><name>Other</name><state>running</state></service>").is_err());
        assert!(
            app_info(b"<service><name>YouTube</name><state>installable</state></service>").is_err()
        );
        let app =
            app_info(b"<service><name>YouTube</name><state>stopped</state></service>").unwrap();
        assert!(app.screen.is_empty());
    }

    #[test]
    fn endpoints_stay_on_the_selected_private_ip_without_credentials_or_redirects() {
        let host = "192.168.1.20".parse().unwrap();
        assert!(valid_endpoint(
            &Url::parse("http://192.168.1.20:8080/ws/apps/YouTube").unwrap(),
            host
        ));
        for url in [
            "http://example.com/apps/YouTube",
            "http://192.168.1.21/apps/YouTube",
            "http://synthetic@192.168.1.20/apps/YouTube",
            "http://192.168.1.20/apps/YouTube?token=synthetic",
            "file:///apps/YouTube",
        ] {
            assert!(!valid_endpoint(&Url::parse(url).unwrap(), host));
        }
        assert!(!valid_endpoint(
            &Url::parse("http://127.0.0.1/apps/YouTube").unwrap(),
            "127.0.0.1".parse().unwrap()
        ));
    }

    #[tokio::test]
    #[ignore = "manual authorized TV playback; set YOUTFAST_TEST_TV_IP"]
    async fn live_youtube_dial_playback_and_controls() {
        let host: IpAddr = std::env::var("YOUTFAST_TEST_TV_IP")
            .unwrap()
            .parse()
            .unwrap();
        let found = discover().await.unwrap();
        eprintln!(
            "DIAL discovery includes requested TV: {}",
            found.iter().any(|device| device.host == host)
        );
        let device = find(host).await.unwrap();
        let Protocol::YouTube { endpoint } = device.protocol else {
            unreachable!()
        };
        let screen = launch(&endpoint, host).await.unwrap();
        let mut lounge = crate::youtube::lounge::Session::connect(&screen)
            .await
            .unwrap();
        lounge.play("jNQXAC9IVRw", 0).await.unwrap();
        eprintln!("Requested video confirmed playing");
        lounge.control(super::super::Control::Pause).await.unwrap();
        eprintln!("Pause sent");
        lounge
            .control(super::super::Control::Forward)
            .await
            .unwrap();
        lounge.control(super::super::Control::Rewind).await.unwrap();
        eprintln!("Relative seeks sent");
        lounge
            .control(super::super::Control::VolumeDown)
            .await
            .unwrap();
        lounge
            .control(super::super::Control::VolumeUp)
            .await
            .unwrap();
        eprintln!("Volume controls sent and restored");
        lounge.control(super::super::Control::Play).await.unwrap();
        stop(&endpoint, host).await.unwrap();
        eprintln!("DIAL stop completed");
    }
}
