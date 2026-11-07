// SPDX-License-Identifier: GPL-3.0-or-later
//! Finding the mediation server and (in parallel) receivers in the local network.
//!
//! Server lookup is a ladder, see [`lookup_order`]:
//!
//! 1. MDM policy ([`policy`]),
//! 2. DNS-SRV `_cometcast._tcp.<search-domain>` ([`srv`]),
//! 3. announcement by the display device over mDNS ([`announce`]),
//! 4. the built-in default list ([`DEFAULT_SERVERS`]).
//!
//! [`discover_servers`] asks the sources and [`merge()`] orders the answers. Discovery of
//! receivers via mDNS runs IN PARALLEL ([`discover_receivers`]), it is not a step of the ladder.
//! See docs/discovery.md.

pub mod address;
pub mod announce;
pub mod display;
pub mod merge;
pub mod nearby;
pub mod policy;
pub mod srv;

use std::time::Duration;

pub use announce::{
    AdvertiseOptions, Advertisement, AnnouncementSource, MdnsError, MdnsSource, ReceiverBrowser,
    ReceiverEvent, ReceiverInfo, advertise_receiver, discover_receivers,
};
pub use display::{
    AnnounceBackend, AnnounceError, DisplayAnnouncement, DisplayAnnouncer, DisplayState,
    MdnsBackend,
};
pub use merge::{Merged, merge};
pub use nearby::{
    NearbyChange, NearbyScreen, NearbySearch, NearbyTracker, RawEvent, RawRecord, RawSink,
    ScreenMode, local_network_available,
};
pub use policy::{
    ModeHint, PolicyFormat, PolicyLocation, PolicyState, ServerPolicy, default_policy_locations,
    load_policy,
};
pub use srv::{SrvResolver, SystemResolver};

/// DNS-SD service type that Comet Cast servers and receivers announce.
pub const SERVICE_TYPE: &str = "_cometcast._tcp";

/// Built-in default servers, used only when every other source found nothing.
// UNCERTAIN: "relay.cometcast.de" is a planned name only. No such host exists yet; it will be set up in the last step of the project.
// It must be replaced or confirmed before release.
pub const DEFAULT_SERVERS: [&str; 1] = ["relay.cometcast.de:443"];

/// Where a server address can come from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServerSource {
    /// Device management (MDM) policy set by an administrator.
    Mdm,
    /// DNS SRV record for `_cometcast._tcp` in the local search domain.
    DnsSrv,
    /// Announcement from the display device (the receiver) itself.
    DisplayAnnouncement,
    /// The built-in [`DEFAULT_SERVERS`] list.
    DefaultList,
}

impl ServerSource {
    /// A short stable name for logs and the command line tool.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Mdm => "mdm",
            Self::DnsSrv => "dns-srv",
            Self::DisplayAnnouncement => "display-announcement",
            Self::DefaultList => "default-list",
        }
    }
}

/// The order in which server sources are asked; the first answer wins.
pub fn lookup_order() -> [ServerSource; 4] {
    [
        ServerSource::Mdm,
        ServerSource::DnsSrv,
        ServerSource::DisplayAnnouncement,
        ServerSource::DefaultList,
    ]
}

/// One server in the result, with where it came from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscoveredServer {
    /// Canonical address, `[scheme://]host:port` (see [`address::normalize_server`]). DNS-SRV
    /// and announced servers always carry `wss://` or `ws://` (see [`address::network_server`]).
    pub address: String,
    /// The source that gave it.
    pub source: ServerSource,
    /// Extra text for people (record, device name, caveat). Never needed for decisions.
    pub detail: Option<String>,
    /// Does the connection use TLS? `Some(true)` for `wss`/`https`, `Some(false)` for
    /// `ws`/`http`, `None` when the address has no scheme (policy and default list: the app
    /// decides). For DNS-SRV and announced servers it is always `Some`; `Some(false)` only for
    /// a private IP literal or a `.local` name, and the UI should say so.
    pub tls: Option<bool>,
    /// The IPv4 address the display's announcement resolved to, for an announced server
    /// (`DisplayAnnouncement`) only: the app shows it as a second way in when the announced
    /// server is a host name (the in-house box, decision N40). `None` otherwise.
    pub ip: Option<std::net::Ipv4Addr>,
}

impl DiscoveredServer {
    /// Build an entry; `tls` is read from the scheme of `address`.
    pub fn new(address: String, source: ServerSource, detail: Option<String>) -> Self {
        let tls = match address.split_once("://") {
            Some(("wss" | "https", _)) => Some(true),
            Some(_) => Some(false),
            None => None,
        };
        Self {
            address,
            source,
            detail,
            tls,
            ip: None,
        }
    }

    /// The same entry with the IPv4 address the announcement resolved to.
    pub fn with_ip(mut self, ip: Option<std::net::Ipv4Addr>) -> Self {
        self.ip = ip;
        self
    }
}

/// The built-in list as [`DiscoveredServer`]s.
pub fn default_servers() -> Vec<DiscoveredServer> {
    DEFAULT_SERVERS
        .iter()
        .map(|a| {
            DiscoveredServer::new(
                (*a).to_owned(),
                ServerSource::DefaultList,
                Some("UNCERTAIN: placeholder, this host does not exist yet".into()),
            )
        })
        .collect()
}

/// Settings of [`discover_servers`].
#[derive(Debug, Clone)]
pub struct DiscoverOptions {
    /// Where to look for a policy file; `None` for the places of this operating system.
    pub policy_locations: Option<Vec<PolicyLocation>>,
    /// Ask DNS for SRV records.
    pub dns_srv: bool,
    /// Time limit of one DNS-SRV request. UNCERTAIN: placeholder, not measured.
    pub srv_timeout: Duration,
    /// Listen for announcements over mDNS.
    pub mdns: bool,
    /// How long to listen for announcements. UNCERTAIN: placeholder, not measured.
    pub mdns_wait: Duration,
}

impl Default for DiscoverOptions {
    fn default() -> Self {
        Self {
            policy_locations: None,
            dns_srv: true,
            srv_timeout: Duration::from_secs(3),
            mdns: true,
            mdns_wait: Duration::from_millis(1500),
        }
    }
}

/// The answer of [`discover_servers`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscoveryReport {
    /// Ordered servers, best first, each with its source.
    pub servers: Vec<DiscoveredServer>,
    /// The policy forbids other servers (or is unusable): do not let people type another one.
    pub locked: bool,
    /// Server mode hint of the policy.
    pub mode_hint: Option<ModeHint>,
    /// What was found about policy.
    pub policy: PolicyState,
    /// Notes (failed lookups and so on); no network content.
    pub warnings: Vec<String>,
}

fn load_policy_blocking(opts: &DiscoverOptions) -> PolicyState {
    let locations = opts
        .policy_locations
        .clone()
        .unwrap_or_else(default_policy_locations);
    load_policy(&locations)
}

fn report(
    policy: PolicyState,
    srv: &[DiscoveredServer],
    ann: &[DiscoveredServer],
    mut warnings: Vec<String>,
) -> DiscoveryReport {
    let m = merge(&policy, srv, ann, &default_servers());
    warnings.extend(m.warnings);
    DiscoveryReport {
        servers: m.servers,
        locked: m.locked,
        mode_hint: m.mode_hint,
        policy,
        warnings,
    }
}

/// Only the sources that need no network: the policy file and the built-in list. Fast;
/// meant for a synchronous call at start-up. Blocking (file access).
pub fn discover_servers_offline(opts: &DiscoverOptions) -> DiscoveryReport {
    report(load_policy_blocking(opts), &[], &[], Vec::new())
}

/// Ask all sources and return the ordered list.
///
/// The policy file is read first. If it forbids other servers (or is unusable), the network is
/// not asked at all. Otherwise DNS-SRV and the mDNS announcement browse run at the same time.
/// Nothing here fails as a whole: a source that does not work leaves a warning.
pub async fn discover_servers(opts: &DiscoverOptions) -> DiscoveryReport {
    let resolver = if opts.dns_srv {
        match SystemResolver::new(opts.srv_timeout) {
            Ok(r) => Some(r),
            Err(e) => {
                let mut r = discover_servers_with(opts, None, &MdnsSource).await;
                r.warnings.push(format!("DNS-SRV is not available: {e}"));
                return r;
            }
        }
    } else {
        None
    };
    discover_servers_with(
        opts,
        resolver.as_ref().map(|r| r as &dyn SrvResolver),
        &MdnsSource,
    )
    .await
}

/// [`discover_servers`] with the DNS and mDNS access given by the caller (for tests).
/// `resolver: None` skips DNS-SRV.
pub async fn discover_servers_with(
    opts: &DiscoverOptions,
    resolver: Option<&dyn SrvResolver>,
    announcements: &dyn AnnouncementSource,
) -> DiscoveryReport {
    let o = opts.clone();
    let policy = tokio::task::spawn_blocking(move || load_policy_blocking(&o))
        .await
        .unwrap_or_else(|_| PolicyState::Invalid("policy reader failed".into()));
    let (forbid_override, forbid_announcement) = match &policy {
        PolicyState::Active(p) => (p.forbid_user_override, p.forbid_announcement),
        PolicyState::Invalid(_) => (true, true),
        PolicyState::Absent => (false, false),
    };
    if forbid_override {
        return report(policy, &[], &[], Vec::new());
    }
    let dns = async {
        match resolver {
            Some(r) if opts.dns_srv => {
                srv::lookup_srv_servers(r, opts.srv_timeout, srv::thread_rand_below).await
            }
            _ => srv::SrvOutcome::default(),
        }
    };
    let mdns = async {
        if !opts.mdns || forbid_announcement {
            return (Vec::new(), Vec::new());
        }
        match announcements.browse(opts.mdns_wait).await {
            Ok(list) => {
                let servers = list
                    .into_iter()
                    .filter_map(|r| {
                        let address = r.announced_server?;
                        // IPv4 addresses come first in `addresses`.
                        let ip = r.addresses.iter().find_map(|a| match a {
                            std::net::IpAddr::V4(v) => Some(*v),
                            std::net::IpAddr::V6(_) => None,
                        });
                        Some(
                            DiscoveredServer::new(
                                address,
                                ServerSource::DisplayAnnouncement,
                                Some(format!("announced by {}", r.name)),
                            )
                            .with_ip(ip),
                        )
                    })
                    .collect();
                (servers, Vec::new())
            }
            Err(e) => (Vec::new(), vec![format!("announcement browse failed: {e}")]),
        }
    };
    let (dns, (ann, ann_warnings)) = tokio::join!(dns, mdns);
    let mut warnings = dns.warnings;
    warnings.extend(ann_warnings);
    report(policy, &dns.servers, &ann, warnings)
}

#[cfg(test)]
mod tests {
    use super::*;
    use srv::{ResolveError, SrvRecord};
    use std::path::Path;

    #[test]
    fn lookup_order_is_mdm_srv_display_default() {
        assert_eq!(
            lookup_order(),
            [
                ServerSource::Mdm,
                ServerSource::DnsSrv,
                ServerSource::DisplayAnnouncement,
                ServerSource::DefaultList
            ]
        );
    }

    #[test]
    fn service_type_and_default_list() {
        assert_eq!(SERVICE_TYPE, "_cometcast._tcp");
        assert_eq!(DEFAULT_SERVERS.len(), 1);
        for d in DEFAULT_SERVERS {
            assert!(address::normalize_server(d, address::Trust::Admin).is_ok());
        }
    }

    struct FakeDns(Vec<SrvRecord>);
    #[async_trait::async_trait]
    impl SrvResolver for FakeDns {
        fn search_domains(&self) -> Vec<String> {
            vec!["corp.example".into()]
        }
        async fn lookup_srv(&self, _: &str) -> Result<Vec<SrvRecord>, ResolveError> {
            Ok(self.0.clone())
        }
    }
    struct FakeMdns(Result<Vec<ReceiverInfo>, MdnsError>);
    #[async_trait::async_trait]
    impl AnnouncementSource for FakeMdns {
        async fn browse(&self, _: Duration) -> Result<Vec<ReceiverInfo>, MdnsError> {
            self.0.clone()
        }
    }

    fn receiver(name: &str, srv: Option<&str>) -> ReceiverInfo {
        ReceiverInfo {
            instance: name.into(),
            name: name.into(),
            addresses: vec![],
            port: 9,
            announced_server: srv.map(str::to_owned),
        }
    }

    #[test]
    fn tls_flag_follows_the_scheme() {
        let t = |a: &str| DiscoveredServer::new(a.into(), ServerSource::Mdm, None).tls;
        assert_eq!(t("wss://a.example:1"), Some(true));
        assert_eq!(t("https://a.example"), Some(true));
        assert_eq!(t("ws://10.0.0.1:1"), Some(false));
        assert_eq!(t("a.example:443"), None);
    }

    fn opts_with(policy: Option<&Path>) -> DiscoverOptions {
        DiscoverOptions {
            policy_locations: Some(
                policy
                    .map(|p| vec![policy::explicit_location(p)])
                    .unwrap_or_default(),
            ),
            ..DiscoverOptions::default()
        }
    }

    fn dns() -> FakeDns {
        FakeDns(vec![SrvRecord {
            priority: 0,
            weight: 0,
            port: 443,
            target: "relay.corp.example.".into(),
        }])
    }

    #[tokio::test]
    async fn all_sources_without_policy() {
        let m = FakeMdns(Ok(vec![
            receiver("tv1", Some("10.1.1.1:8443")),
            receiver("tv2", None),
        ]));
        let r = discover_servers_with(&opts_with(None), Some(&dns()), &m).await;
        let got: Vec<(&str, ServerSource)> = r
            .servers
            .iter()
            .map(|s| (s.address.as_str(), s.source))
            .collect();
        assert_eq!(
            got,
            [
                ("wss://relay.corp.example:443", ServerSource::DnsSrv),
                ("10.1.1.1:8443", ServerSource::DisplayAnnouncement),
                ("relay.cometcast.de:443", ServerSource::DefaultList),
            ]
        );
        assert!(!r.locked);
        assert_eq!(r.policy, PolicyState::Absent);
    }

    #[tokio::test]
    async fn an_announced_server_carries_the_ipv4_address_of_the_announcement() {
        let mut box_ = receiver("box", Some("wss://cometcast.box:443"));
        // Addresses come IPv4 first (`receiver_from_parts` sorts them).
        box_.addresses = vec!["192.168.1.20".parse().unwrap(), "fd00::2".parse().unwrap()];
        let mut only_v6 = receiver("v6", Some("wss://v6.example:443"));
        only_v6.addresses = vec!["fd00::3".parse().unwrap()];
        let m = FakeMdns(Ok(vec![box_, only_v6]));
        let r = discover_servers_with(&opts_with(None), None, &m).await;
        let ips: Vec<(&str, Option<String>)> = r
            .servers
            .iter()
            .filter(|s| s.source == ServerSource::DisplayAnnouncement)
            .map(|s| (s.address.as_str(), s.ip.map(|i| i.to_string())))
            .collect();
        assert_eq!(
            ips,
            [
                ("wss://cometcast.box:443", Some("192.168.1.20".to_owned())),
                ("wss://v6.example:443", None)
            ]
        );
        // Other sources carry none.
        assert!(
            r.servers
                .iter()
                .filter(|s| s.source != ServerSource::DisplayAnnouncement)
                .all(|s| s.ip.is_none())
        );
    }

    #[tokio::test]
    async fn failing_sources_leave_warnings_and_the_default() {
        let m = FakeMdns(Err(MdnsError("no multicast".into())));
        let r = discover_servers_with(&opts_with(None), None, &m).await;
        assert_eq!(r.servers.len(), 1);
        assert_eq!(r.servers[0].source, ServerSource::DefaultList);
        assert!(r.warnings[0].contains("no multicast"));
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn policy_file_is_first_and_can_lock() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("policy.json");
        let write = |body: &str| {
            std::fs::write(&p, body).unwrap();
            std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o644)).unwrap();
        };
        let m = FakeMdns(Ok(vec![receiver("tv1", Some("10.1.1.1:8443"))]));

        write(
            r#"{"servers":["wss://mdm.example:443"],"forbid_default_list":true,"mode":"internal"}"#,
        );
        let r = discover_servers_with(&opts_with(Some(&p)), Some(&dns()), &m).await;
        let a: Vec<&str> = r.servers.iter().map(|s| s.address.as_str()).collect();
        assert_eq!(
            a,
            [
                "wss://mdm.example:443",
                "wss://relay.corp.example:443",
                "10.1.1.1:8443"
            ]
        );
        assert_eq!(r.servers[0].source, ServerSource::Mdm);
        assert_eq!(r.mode_hint, Some(ModeHint::Internal));
        assert!(!r.locked);

        write(r#"{"servers":["wss://mdm.example:443"],"forbid_user_override":true}"#);
        let r = discover_servers_with(&opts_with(Some(&p)), Some(&dns()), &m).await;
        assert_eq!(r.servers.len(), 1);
        assert!(r.locked);

        write(r#"{"forbid_announcement":true}"#);
        let r = discover_servers_with(&opts_with(Some(&p)), Some(&dns()), &m).await;
        assert!(
            r.servers
                .iter()
                .all(|s| s.source != ServerSource::DisplayAnnouncement)
        );

        write("{broken");
        let r = discover_servers_with(&opts_with(Some(&p)), Some(&dns()), &m).await;
        assert!(r.servers.is_empty() && r.locked);
        assert!(!r.warnings.is_empty());
    }

    #[test]
    fn offline_report_is_policy_plus_defaults() {
        let r = discover_servers_offline(&opts_with(None));
        assert_eq!(r.servers, default_servers());
    }
}
