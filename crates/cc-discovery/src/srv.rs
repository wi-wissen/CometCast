// SPDX-License-Identifier: GPL-3.0-or-later
//! DNS-SRV lookup of `_cometcast._tcp.<search-domain>` (RFC 2782).
//!
//! The DNS access sits behind the [`SrvResolver`] trait so that tests can use a fake. The real
//! one, [`SystemResolver`], uses `hickory-resolver` with the system configuration (name servers
//! and search domains from `/etc/resolv.conf` on Unix, the registry on Windows, system settings
//! on macOS).
//!
//! UNCERTAIN: Android (`getDomains` of `DnsResolver`) and iOS (`lb._dns-sd`, RFC 6763 section
//! 11) are NOT covered; on those systems the system configuration read by hickory may be empty
//! or missing and the lookup then finds nothing (see docs/measurement-points.md, A8).
//!
//! DNS answers are untrusted. At most [`MAX_SRV_RECORDS`] records are looked at, targets must be
//! valid host names that pass the network rules of [`crate::address`], and the whole lookup is
//! limited by a timeout.

use std::time::Duration;

use async_trait::async_trait;
use hickory_resolver::TokioResolver;
use hickory_resolver::proto::rr::{RData, RecordType};

use crate::address::{Trust, normalize_server, validate_hostname};
use crate::{DiscoveredServer, ServerSource};

/// DNS name prefix of the service.
pub const SRV_PREFIX: &str = "_cometcast._tcp";
/// Most records taken from one answer.
pub const MAX_SRV_RECORDS: usize = 32;
/// Most servers returned from one lookup.
pub const MAX_SRV_SERVERS: usize = 8;
/// Most search domains asked.
pub const MAX_SEARCH_DOMAINS: usize = 4;

/// One SRV record as far as this crate needs it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SrvRecord {
    /// Lower is tried first.
    pub priority: u16,
    /// Relative share among records of the same priority.
    pub weight: u16,
    /// TCP port.
    pub port: u16,
    /// Target host name (may end with a dot; `.` alone means "service not available").
    pub target: String,
}

/// A failed DNS lookup.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ResolveError {
    /// There is no such record (NXDOMAIN or NODATA). Not worth a warning.
    #[error("no records")]
    NoRecords,
    /// The lookup did not finish in time.
    #[error("DNS lookup timed out")]
    Timeout,
    /// Anything else.
    #[error("DNS lookup failed: {0}")]
    Failed(String),
}

/// What the SRV lookup needs from DNS.
#[async_trait]
pub trait SrvResolver: Send + Sync {
    /// Search domains of this system, most important first.
    fn search_domains(&self) -> Vec<String>;
    /// The SRV records of the fully qualified `name` (e.g. `_cometcast._tcp.example.org`).
    async fn lookup_srv(&self, name: &str) -> Result<Vec<SrvRecord>, ResolveError>;
}

/// The system's DNS resolver (hickory).
pub struct SystemResolver {
    resolver: TokioResolver,
    domains: Vec<String>,
}

impl SystemResolver {
    /// Read the system configuration. `timeout` is the time for one DNS request; one attempt
    /// per name server is made so that a dead server cannot stall the lookup for long.
    pub fn new(timeout: Duration) -> Result<Self, ResolveError> {
        let (config, mut opts) = hickory_resolver::system_conf::read_system_conf()
            .map_err(|e| ResolveError::Failed(e.to_string()))?;
        let domains = config
            .domain()
            .into_iter()
            .chain(config.search())
            .map(|n| n.to_ascii())
            .collect();
        opts.timeout = timeout;
        opts.attempts = 1;
        let mut builder = TokioResolver::builder_with_config(
            config,
            hickory_resolver::net::runtime::TokioRuntimeProvider::default(),
        );
        *builder.options_mut() = opts;
        let resolver = builder
            .build()
            .map_err(|e| ResolveError::Failed(e.to_string()))?;
        Ok(Self { resolver, domains })
    }
}

#[async_trait]
impl SrvResolver for SystemResolver {
    fn search_domains(&self) -> Vec<String> {
        self.domains.clone()
    }

    async fn lookup_srv(&self, name: &str) -> Result<Vec<SrvRecord>, ResolveError> {
        // The trailing dot makes the name fully qualified: no search list is applied again.
        let fqdn = format!("{}.", name.trim_end_matches('.'));
        let lookup = self
            .resolver
            .lookup(fqdn.as_str(), RecordType::SRV)
            .await
            .map_err(|e| {
                if e.is_no_records_found() || e.is_nx_domain() {
                    ResolveError::NoRecords
                } else {
                    ResolveError::Failed(e.to_string())
                }
            })?;
        Ok(lookup
            .answers()
            .iter()
            .filter_map(|r| match &r.data {
                RData::SRV(s) => Some(SrvRecord {
                    priority: s.priority,
                    weight: s.weight,
                    port: s.port,
                    target: s.target.to_ascii(),
                }),
                _ => None,
            })
            .take(MAX_SRV_RECORDS)
            .collect())
    }
}

/// Order records as RFC 2782 asks: by priority, and inside one priority by weighted random
/// choice. `rand_below(n)` must return a number in `0..n` (it is only called with `n >= 1`).
pub fn order_srv(
    mut records: Vec<SrvRecord>,
    mut rand_below: impl FnMut(u32) -> u32,
) -> Vec<SrvRecord> {
    records.sort_by_key(|r| r.priority); // stable
    let mut out = Vec::with_capacity(records.len());
    let mut i = 0;
    while i < records.len() {
        let prio = records[i].priority;
        let end = records[i..]
            .iter()
            .position(|r| r.priority != prio)
            .map_or(records.len(), |n| i + n);
        let mut group: Vec<SrvRecord> = records[i..end].to_vec();
        // RFC 2782: records with weight 0 are put at the beginning of the list.
        group.sort_by_key(|r| r.weight != 0);
        while !group.is_empty() {
            let total: u32 = group.iter().map(|r| u32::from(r.weight)).sum();
            let pick = rand_below(total + 1).min(total);
            let mut running = 0;
            let mut chosen = group.len() - 1;
            for (idx, r) in group.iter().enumerate() {
                running += u32::from(r.weight);
                if running >= pick {
                    chosen = idx;
                    break;
                }
            }
            out.push(group.remove(chosen));
        }
        i = end;
    }
    out
}

/// Result of [`lookup_srv_servers`].
#[derive(Debug, Default, PartialEq, Eq)]
pub struct SrvOutcome {
    /// Validated servers in the order they should be tried.
    pub servers: Vec<DiscoveredServer>,
    /// Things worth telling the user (never DNS content).
    pub warnings: Vec<String>,
}

/// Look up `_cometcast._tcp.<domain>` for each search domain (at most
/// [`MAX_SEARCH_DOMAINS`]) and return the servers of the first domain that has usable answers.
/// `timeout` limits each request as a whole. Never panics on hostile answers: bad records are
/// dropped.
pub async fn lookup_srv_servers(
    resolver: &dyn SrvResolver,
    timeout: Duration,
    rand_below: impl FnMut(u32) -> u32,
) -> SrvOutcome {
    let mut rand_below = rand_below;
    let mut out = SrvOutcome::default();
    let mut seen: Vec<String> = Vec::new();
    for domain in resolver.search_domains() {
        if seen.len() >= MAX_SEARCH_DOMAINS {
            break;
        }
        let Ok(domain) = validate_hostname(&domain) else {
            continue;
        };
        if seen.contains(&domain) {
            continue;
        }
        seen.push(domain.clone());
        let name = format!("{SRV_PREFIX}.{domain}");
        let records = match tokio::time::timeout(timeout, resolver.lookup_srv(&name)).await {
            Err(_) | Ok(Err(ResolveError::Timeout)) => {
                out.warnings
                    .push(format!("DNS-SRV lookup for {name} timed out"));
                continue;
            }
            Ok(Err(ResolveError::NoRecords)) => continue,
            Ok(Err(e)) => {
                out.warnings.push(format!("DNS-SRV lookup for {name}: {e}"));
                continue;
            }
            Ok(Ok(r)) => r,
        };
        let records: Vec<SrvRecord> = records.into_iter().take(MAX_SRV_RECORDS).collect();
        let unavailable = records.iter().any(|r| r.target == ".");
        if unavailable && records.len() == 1 {
            // RFC 2782: a single record with target "." says the service is not available here.
            out.warnings
                .push(format!("DNS-SRV {name} says the service is not available"));
            break;
        }
        let mut dropped = 0usize;
        for r in order_srv(records, &mut rand_below) {
            let cand = normalize_server(
                &format!("{}:{}", r.target.trim_end_matches('.'), r.port),
                Trust::Network,
            );
            match cand {
                Ok(address) if !out.servers.iter().any(|s| s.address == address) => {
                    if out.servers.len() < MAX_SRV_SERVERS {
                        out.servers.push(DiscoveredServer {
                            address,
                            source: ServerSource::DnsSrv,
                            detail: Some(format!(
                                "{name}, priority {}, weight {}",
                                r.priority, r.weight
                            )),
                        });
                    }
                }
                Ok(_) => {}
                Err(_) => dropped += 1,
            }
        }
        if dropped > 0 {
            out.warnings.push(format!(
                "{dropped} DNS-SRV record(s) for {name} were refused"
            ));
        }
        if !out.servers.is_empty() {
            break;
        }
    }
    out
}

/// Production random numbers for [`order_srv`].
pub fn thread_rand_below(n: u32) -> u32 {
    use rand::Rng;
    rand::rng().random_range(0..n.max(1))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn rec(p: u16, w: u16, port: u16, t: &str) -> SrvRecord {
        SrvRecord {
            priority: p,
            weight: w,
            port,
            target: t.into(),
        }
    }

    #[derive(Default)]
    struct Fake {
        domains: Vec<String>,
        answers: HashMap<String, Result<Vec<SrvRecord>, ResolveError>>,
        hang: bool,
    }

    #[async_trait]
    impl SrvResolver for Fake {
        fn search_domains(&self) -> Vec<String> {
            self.domains.clone()
        }
        async fn lookup_srv(&self, name: &str) -> Result<Vec<SrvRecord>, ResolveError> {
            if self.hang {
                std::future::pending::<()>().await;
            }
            self.answers
                .get(name)
                .cloned()
                .unwrap_or(Err(ResolveError::NoRecords))
        }
    }

    fn first(_n: u32) -> u32 {
        0
    }

    async fn run(f: &Fake) -> SrvOutcome {
        lookup_srv_servers(f, Duration::from_millis(200), first).await
    }

    #[test]
    fn priority_orders_before_weight() {
        let out = order_srv(
            vec![
                rec(20, 5, 1, "c.x"),
                rec(10, 5, 2, "a.x"),
                rec(10, 0, 3, "b.x"),
            ],
            first,
        );
        let ports: Vec<u16> = out.iter().map(|r| r.port).collect();
        assert_eq!(ports[2], 1, "priority 20 last");
        assert_eq!(ports.len(), 3);
    }

    #[test]
    fn zero_weight_first_and_weighted_pick() {
        // rand 0 picks the first of the list; zero-weight records come first (RFC 2782).
        let recs = vec![
            rec(1, 10, 1, "a.x"),
            rec(1, 0, 2, "b.x"),
            rec(1, 30, 3, "c.x"),
        ];
        let out = order_srv(recs.clone(), first);
        assert_eq!(out[0].port, 2);
        // rand = max picks the last running sum: the heaviest remaining at the end of the list.
        let out = order_srv(recs.clone(), |n| n - 1);
        assert_eq!(out.len(), 3);
        // Weight proportions: with a fixed sequence of random values the choice is exact.
        let mut seq = [11u32, 0].into_iter();
        let out = order_srv(
            vec![rec(1, 10, 1, "a.x"), rec(1, 30, 3, "c.x")],
            move |_| seq.next().unwrap_or(0),
        );
        assert_eq!(out[0].port, 3, "11 > 10 selects the second record");
    }

    #[test]
    fn weighted_distribution_is_roughly_proportional() {
        let mut state = 12345u64;
        let mut firsts = 0;
        for _ in 0..2000 {
            let out = order_srv(vec![rec(1, 1, 1, "a.x"), rec(1, 3, 2, "b.x")], |n| {
                state = state
                    .wrapping_mul(6364136223846793005)
                    .wrapping_add(1442695040888963407);
                ((state >> 33) as u32) % n
            });
            if out[0].port == 2 {
                firsts += 1;
            }
        }
        // RFC 2782 draws from 0..=sum, so b (weight 3 of 4) leads in 3 of 5 cases: about 60 %.
        assert!((1050..1350).contains(&firsts), "{firsts}");
    }

    #[test]
    fn order_srv_survives_extremes() {
        let many: Vec<SrvRecord> = (0..1000).map(|i| rec(i % 3, u16::MAX, 1, "a.x")).collect();
        assert_eq!(order_srv(many, |n| n - 1).len(), 1000);
        assert!(order_srv(vec![], first).is_empty());
        // A misbehaving random source must not panic.
        assert_eq!(order_srv(vec![rec(1, 5, 1, "a.x")], |_| u32::MAX).len(), 1);
    }

    #[tokio::test]
    async fn finds_servers_in_first_search_domain_with_answers() {
        let mut f = Fake {
            domains: vec!["corp.example".into(), "example".into()],
            ..Fake::default()
        };
        f.answers.insert(
            "_cometcast._tcp.corp.example".into(),
            Ok(vec![
                rec(20, 1, 8443, "Backup.Corp.Example."),
                rec(10, 1, 443, "relay.corp.example."),
            ]),
        );
        let o = run(&f).await;
        let a: Vec<&str> = o.servers.iter().map(|s| s.address.as_str()).collect();
        assert_eq!(a, ["relay.corp.example:443", "backup.corp.example:8443"]);
        assert!(o.servers.iter().all(|s| s.source == ServerSource::DnsSrv));
        assert!(o.warnings.is_empty());
    }

    #[tokio::test]
    async fn falls_through_to_next_domain() {
        let mut f = Fake {
            domains: vec!["a.example".into(), "b.example".into()],
            ..Fake::default()
        };
        f.answers.insert(
            "_cometcast._tcp.b.example".into(),
            Ok(vec![rec(0, 0, 1, "x.b.example")]),
        );
        let o = run(&f).await;
        assert_eq!(o.servers.len(), 1);
        assert_eq!(o.servers[0].address, "x.b.example:1");
    }

    #[tokio::test]
    async fn no_domains_no_answers_is_empty_without_warning() {
        assert_eq!(run(&Fake::default()).await, SrvOutcome::default());
        let f = Fake {
            domains: vec!["a.example".into()],
            ..Fake::default()
        };
        assert_eq!(run(&f).await, SrvOutcome::default());
    }

    #[tokio::test]
    async fn timeout_and_errors_become_warnings() {
        let f = Fake {
            domains: vec!["a.example".into()],
            hang: true,
            ..Fake::default()
        };
        let o = run(&f).await;
        assert!(o.servers.is_empty());
        assert_eq!(o.warnings.len(), 1);
        let mut f = Fake {
            domains: vec!["a.example".into()],
            ..Fake::default()
        };
        f.answers.insert(
            "_cometcast._tcp.a.example".into(),
            Err(ResolveError::Failed("boom".into())),
        );
        let o = run(&f).await;
        assert!(o.warnings[0].contains("boom"));
    }

    #[tokio::test]
    async fn target_dot_means_unavailable() {
        let mut f = Fake {
            domains: vec!["a.example".into(), "b.example".into()],
            ..Fake::default()
        };
        f.answers.insert(
            "_cometcast._tcp.a.example".into(),
            Ok(vec![rec(0, 0, 1, ".")]),
        );
        f.answers.insert(
            "_cometcast._tcp.b.example".into(),
            Ok(vec![rec(0, 0, 1, "x.b.example")]),
        );
        let o = run(&f).await;
        assert!(
            o.servers.is_empty(),
            "the explicit refusal stops the search"
        );
        assert_eq!(o.warnings.len(), 1);
    }

    #[tokio::test]
    async fn hostile_answers_are_filtered() {
        let mut recs = vec![
            rec(0, 0, 0, "port-zero.example"),
            rec(0, 0, 80, "127.0.0.1"),
            rec(0, 0, 80, "localhost"),
            rec(0, 0, 80, "8.8.8.8"),
            rec(0, 0, 80, "bad_name.example"),
            rec(0, 0, 80, "evil.example/ws"),
            rec(0, 0, 80, "user@evil.example"),
            rec(0, 0, 80, &"a".repeat(64)),
            rec(0, 0, 80, "sp ace.example"),
            rec(0, 0, 80, "wss://x.example"),
            rec(0, 0, 80, ""),
            rec(0, 0, 80, "good.example."),
            rec(0, 0, 80, "good.example"),
        ];
        // A flood of valid-looking records is capped.
        recs.extend((0..500).map(|i| rec(1, 1, 1000 + i, &format!("h{i}.example"))));
        let mut f = Fake {
            domains: vec!["a.example".into()],
            ..Fake::default()
        };
        f.answers
            .insert("_cometcast._tcp.a.example".into(), Ok(recs));
        let o = run(&f).await;
        assert_eq!(o.servers[0].address, "good.example:80");
        assert!(o.servers.len() <= MAX_SRV_SERVERS);
        assert!(!o.servers.iter().any(|s| s.address.contains("127.0.0.1")));
        assert!(o.warnings.iter().any(|w| w.contains("refused")));
    }

    #[tokio::test]
    async fn hostile_search_domains_are_ignored_and_capped() {
        let mut f = Fake {
            domains: vec![
                "bad domain".into(),
                "".into(),
                "a_b.example".into(),
                "../x".into(),
                "ok1.example".into(),
                "ok1.example.".into(),
                "ok2.example".into(),
                "ok3.example".into(),
                "ok4.example".into(),
                "ok5.example".into(),
            ],
            ..Fake::default()
        };
        // Only the fifth valid domain would answer: the cap keeps us from asking it.
        f.answers.insert(
            "_cometcast._tcp.ok5.example".into(),
            Ok(vec![rec(0, 0, 1, "x.example")]),
        );
        assert!(run(&f).await.servers.is_empty());
        f.answers.insert(
            "_cometcast._tcp.ok4.example".into(),
            Ok(vec![rec(0, 0, 1, "y.example")]),
        );
        assert_eq!(run(&f).await.servers[0].address, "y.example:1");
    }

    #[tokio::test]
    async fn system_resolver_builds_or_reports_an_error() {
        // The sandbox may have no resolv.conf: both outcomes are fine, neither may panic.
        match SystemResolver::new(Duration::from_millis(50)) {
            Ok(r) => {
                let _ = r.search_domains();
            }
            Err(e) => assert!(matches!(e, ResolveError::Failed(_))),
        }
    }
}
