//! Local DIAL discovery and app lifecycle. Discovery only performs reads.
//! Samsung also advertises AirPlay when SSDP is filtered; that is a discovery
//! hint, never permission to send media through AirPlay or to launch an app.

use std::collections::HashSet;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
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
    // Asking the TV itself returns its real DIAL address (LG, Roku and others
    // use their own ports), and a reply from the address asked passes
    // stateful firewalls that drop replies to a multicast search.
    let socket = bind(None).await?;
    for location in search(&socket, SocketAddr::new(host, 1900), Duration::from_secs(2))
        .await
        .into_iter()
        .filter(|(source, _)| *source == host)
        .map(|(_, location)| location)
    {
        if let Ok(endpoint) = described(&client, host, location).await {
            return Ok(device(host, endpoint));
        }
    }
    for (port, path) in [
        (8080, "/ws/apps/YouTube"),
        (8008, "/apps/YouTube"),
        (8060, "/dial/YouTube"),
    ] {
        let endpoint = endpoint(host, port, path)?;
        if info(&client, &endpoint, host).await.is_ok() {
            return Ok(device(host, endpoint));
        }
    }
    Err(Error::NoYouTube)
}

fn device(host: IpAddr, endpoint: Url) -> Device {
    Device {
        host,
        name: "YouTube on TV".into(),
        protocol: Protocol::YouTube { endpoint },
    }
}

/// Follows a device description to its DIAL application URL.
async fn described(client: &Client, host: IpAddr, location: Url) -> Result<Url, Error> {
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
    info(client, &endpoint, host).await?;
    Ok(endpoint)
}

fn endpoint(host: IpAddr, port: u16, path: &str) -> Result<Url, Error> {
    let mut url = Url::parse("http://localhost").map_err(|_| Error::InvalidRequest)?;
    url.set_ip_host(host).map_err(|_| Error::InvalidRequest)?;
    url.set_port(Some(port))
        .map_err(|_| Error::InvalidRequest)?;
    url.set_path(path);
    Ok(url)
}

/// The screen of a YouTube app the TV already shows, so a send joins it
/// instead of closing what is on screen.
pub(super) async fn running_screen(endpoint: &Url, host: IpAddr) -> Result<Option<String>, Error> {
    let app = info(&client()?, endpoint, host).await?;
    Ok((app.state == "running" && !app.screen.is_empty()).then_some(app.screen))
}

pub(super) async fn launch(endpoint: &Url, host: IpAddr) -> Result<String, Error> {
    let client = client()?;
    let app = info(&client, endpoint, host).await?;
    // A previous remote can leave a queued video behind the TV's login screen.
    // Start a fresh receiver before creating our Lounge session. DIAL makes the
    // stop link optional: without one, the launch below reaches the running app.
    if app.state != "stopped" && app.stop.is_some() {
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
        .join(app.stop.as_deref().ok_or(Error::Unsupported)?)
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
    let (hints, descriptions) = tokio::join!(mdns_hosts(), ssdp_descriptions());
    let client = client()?;
    let mut found = Vec::new();
    let mut tasks = tokio::task::JoinSet::new();
    for (host, name) in hints.unwrap_or_default().into_iter().take(32) {
        tasks.spawn(async move {
            find(host).await.map(|mut device| {
                device.name = name;
                device
            })
        });
    }
    for (host, location) in descriptions.into_iter().take(32) {
        let client = client.clone();
        tasks.spawn(async move { Ok(device(host, described(&client, host, location).await?)) });
    }
    while let Some(result) = tasks.join_next().await {
        if let Ok(Ok(device)) = result
            && !found.iter().any(|known: &Device| known.host == device.host)
        {
            found.push(device);
        }
    }
    Ok(found)
}

/// Addresses announced over mDNS, probed for a YouTube DIAL endpoint. Samsung
/// and LG announce AirPlay; Android TV and Google TV announce Google Cast.
async fn mdns_hosts() -> Result<Vec<(IpAddr, String)>, Error> {
    let daemon = Mdns(ServiceDaemon::new().map_err(|_| Error::Unavailable)?);
    let airplay = daemon
        .0
        .browse("_airplay._tcp.local.")
        .map_err(|_| Error::Unavailable)?;
    let cast = daemon
        .0
        .browse("_googlecast._tcp.local.")
        .map_err(|_| Error::Unavailable)?;
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    let mut hosts = Vec::new();
    loop {
        let event = tokio::select! {
            event = tokio::time::timeout_at(deadline, airplay.recv_async()) => event,
            event = tokio::time::timeout_at(deadline, cast.recv_async()) => event,
        };
        let Ok(Ok(event)) = event else { break };
        if let ServiceEvent::ServiceResolved(info) = event {
            let name = info
                .get_property_val_str("fn")
                .or_else(|| info.get_property_val_str("model"))
                .unwrap_or("YouTube on TV");
            let name: String = name.chars().filter(|c| !c.is_control()).take(80).collect();
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

const SSDP: SocketAddr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(239, 255, 255, 250)), 1900);

/// One socket per local IPv4 interface: with a VPN or container bridge the
/// default route may not be the TV's network.
async fn ssdp_descriptions() -> Vec<(IpAddr, Url)> {
    let mut addresses: Vec<Ipv4Addr> = if_addrs::get_if_addrs()
        .unwrap_or_default()
        .into_iter()
        .filter_map(|interface| match interface.ip() {
            IpAddr::V4(ip) if !interface.is_loopback() && local_address(IpAddr::V4(ip)) => Some(ip),
            _ => None,
        })
        .collect();
    // Home networks first: container bridges (172.16/12) can outnumber them.
    addresses.sort_by_key(|ip| {
        (
            !ip.octets().starts_with(&[192, 168]),
            ip.octets()[0] != 10,
            *ip,
        )
    });
    addresses.dedup();
    addresses.truncate(16);
    let mut sockets = Vec::new();
    for address in addresses {
        if let Ok(socket) = bind(Some(address)).await {
            sockets.push(socket);
        }
    }
    if sockets.is_empty()
        && let Ok(socket) = bind(None).await
    {
        sockets.push(socket);
    }
    let mut searches = tokio::task::JoinSet::new();
    for socket in sockets {
        searches.spawn(async move { search(&socket, SSDP, Duration::from_secs(4)).await });
    }
    let mut seen = HashSet::new();
    let mut found = Vec::new();
    while let Some(result) = searches.join_next().await {
        for (host, location) in result.unwrap_or_default() {
            if found.len() < 32 && seen.insert(location.clone()) {
                found.push((host, location));
            }
        }
    }
    found
}

async fn bind(interface: Option<Ipv4Addr>) -> Result<tokio::net::UdpSocket, Error> {
    let socket = socket2::Socket::new(
        socket2::Domain::IPV4,
        socket2::Type::DGRAM,
        Some(socket2::Protocol::UDP),
    )
    .map_err(|_| Error::Unavailable)?;
    let address = interface.unwrap_or(Ipv4Addr::UNSPECIFIED);
    if let Some(interface) = interface {
        socket
            .set_multicast_if_v4(&interface)
            .map_err(|_| Error::Unavailable)?;
    }
    socket
        .set_multicast_ttl_v4(2)
        .map_err(|_| Error::Unavailable)?;
    socket
        .set_nonblocking(true)
        .map_err(|_| Error::Unavailable)?;
    socket
        .bind(&SocketAddr::new(IpAddr::V4(address), 0).into())
        .map_err(|_| Error::Unavailable)?;
    tokio::net::UdpSocket::from_std(socket.into()).map_err(|_| Error::Unavailable)
}

/// Sends the search three times, since one lost datagram would hide a TV.
async fn search(
    socket: &tokio::net::UdpSocket,
    target: SocketAddr,
    wait: Duration,
) -> Vec<(IpAddr, Url)> {
    let message = format!(
        "M-SEARCH * HTTP/1.1\r\nHOST: {target}\r\nMAN: \"ssdp:discover\"\r\nMX: 2\r\nST: urn:dial-multiscreen-org:service:dial:1\r\n\r\n"
    );
    let start = tokio::time::Instant::now();
    let deadline = start + wait;
    let resend = [
        Duration::ZERO,
        Duration::from_millis(400),
        Duration::from_millis(1000),
    ];
    let mut sent = 0;
    let mut buffer = [0; 16 * 1024];
    let mut found = Vec::new();
    loop {
        if sent < resend.len() && tokio::time::Instant::now() >= start + resend[sent] {
            let _ = socket.send_to(message.as_bytes(), target).await;
            sent += 1;
        }
        let until = resend
            .get(sent)
            .map_or(deadline, |delay| (start + *delay).min(deadline));
        match tokio::time::timeout_at(until, socket.recv_from(&mut buffer)).await {
            Ok(Ok((length, source))) => {
                if local_address(source.ip()) && found.len() < 32 {
                    for location in locations(&buffer[..length], source.ip()) {
                        if !found.iter().any(|(_, known)| *known == location) {
                            found.push((source.ip(), location));
                        }
                    }
                }
            }
            Ok(Err(_)) if sent >= resend.len() => break,
            Ok(Err(_)) => {}
            Err(_) if tokio::time::Instant::now() >= deadline => break,
            Err(_) => {}
        }
    }
    found
}

/// Description URLs of a search reply, only on the replying address.
fn locations(reply: &[u8], source: IpAddr) -> Vec<Url> {
    String::from_utf8_lossy(reply)
        .lines()
        .filter_map(|line| {
            let (key, value) = line.split_once(':')?;
            if !key.eq_ignore_ascii_case("location") {
                return None;
            }
            let location = Url::parse(value.trim()).ok()?;
            (location.scheme() == "http"
                && location.host_str().and_then(|h| h.parse::<IpAddr>().ok()) == Some(source)
                && location.username().is_empty()
                && location.password().is_none()
                && location.query().is_none()
                && location.fragment().is_none())
            .then_some(location)
        })
        .collect()
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

    #[test]
    fn search_replies_only_yield_plain_descriptions_on_the_replying_address() {
        let source: IpAddr = "192.168.1.20".parse().unwrap();
        let reply = b"HTTP/1.1 200 OK\r\nCACHE-CONTROL: max-age=1800\r\nlocation: http://192.168.1.20:36866/description.xml\r\nST: urn:dial-multiscreen-org:service:dial:1\r\n\r\n";
        assert_eq!(
            locations(reply, source),
            [Url::parse("http://192.168.1.20:36866/description.xml").unwrap()]
        );
        for location in [
            "http://192.168.1.21:8060/dial/dd.xml",
            "https://192.168.1.20/description.xml",
            "http://synthetic@192.168.1.20/description.xml",
            "http://192.168.1.20/description.xml?token=synthetic",
        ] {
            let reply = format!("HTTP/1.1 200 OK\r\nLOCATION: {location}\r\n\r\n");
            assert!(locations(reply.as_bytes(), source).is_empty(), "{location}");
        }
    }

    #[test]
    fn an_app_without_a_stop_link_is_still_a_target() {
        // DIAL makes the stop link optional; launch then skips the stop.
        let app = app_info(
            b"<service><name>YouTube</name><state>running</state><additionalData><screenId>synthetic</screenId></additionalData></service>",
        )
        .unwrap();
        assert!(app.stop.is_none());
        assert_eq!(app.screen, "synthetic");
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
