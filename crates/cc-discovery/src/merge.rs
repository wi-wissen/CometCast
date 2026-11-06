// SPDX-License-Identifier: GPL-3.0-or-later
//! Putting the answers of all sources into one ordered list. Pure: no I/O.

use crate::policy::{ModeHint, PolicyState};
use crate::{DiscoveredServer, ServerSource};

/// The merged answer.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Merged {
    /// Servers in the order they should be offered; the first one is the pre-fill.
    pub servers: Vec<DiscoveredServer>,
    /// The policy forbids other servers than its own (or is unusable): the UI must not let
    /// people type another server.
    pub locked: bool,
    /// Mode hint of the policy.
    pub mode_hint: Option<ModeHint>,
    /// Notes for the person running the tool. Never contain network or file content.
    pub warnings: Vec<String>,
}

/// Merge the sources in the order of [`crate::lookup_order`]:
///
/// 1. MDM policy servers.
/// 2. `srv`: DNS-SRV servers.
/// 3. `announced`: servers announced by display devices (dropped when the policy has
///    `forbid_announcement`).
/// 4. `defaults`: the built-in list (dropped when the policy has `forbid_default_list`).
///
/// Rules:
/// * With `forbid_user_override` only the policy servers are returned and the result is
///   `locked`; `srv`, `announced` and `defaults` are ignored.
/// * A policy file that exists but is invalid ([`PolicyState::Invalid`]) fails closed: no
///   servers, `locked`, one warning.
/// * An address appears once, at its first (best) position, compared in lower case.
/// * Inside one source the given order is kept.
pub fn merge(
    policy: &PolicyState,
    srv: &[DiscoveredServer],
    announced: &[DiscoveredServer],
    defaults: &[DiscoveredServer],
) -> Merged {
    let mut out = Merged::default();
    let push = |list: &mut Vec<DiscoveredServer>, s: &DiscoveredServer| {
        let key = s.address.to_ascii_lowercase();
        if !list.iter().any(|o| o.address.to_ascii_lowercase() == key) {
            list.push(s.clone());
        }
    };
    let active = match policy {
        PolicyState::Absent => None,
        PolicyState::Invalid(why) => {
            out.locked = true;
            out.warnings.push(format!(
                "the policy file is unusable ({why}); no server is offered"
            ));
            return out;
        }
        PolicyState::Active(p) => Some(p),
    };
    if let Some(p) = active {
        out.mode_hint = p.mode;
        for a in &p.servers {
            push(
                &mut out.servers,
                &DiscoveredServer {
                    address: a.clone(),
                    source: ServerSource::Mdm,
                    detail: None,
                },
            );
        }
        if p.forbid_user_override {
            out.locked = true;
            if out.servers.is_empty() {
                out.warnings
                    .push("the policy forbids other servers but lists none".into());
            }
            return out;
        }
    }
    let forbid_announcement = active.is_some_and(|p| p.forbid_announcement);
    let forbid_default = active.is_some_and(|p| p.forbid_default_list);
    for s in srv {
        push(&mut out.servers, s);
    }
    if !forbid_announcement {
        for s in announced {
            push(&mut out.servers, s);
        }
    }
    if !forbid_default {
        for s in defaults {
            push(&mut out.servers, s);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::policy::ServerPolicy;

    fn d(a: &str, s: ServerSource) -> DiscoveredServer {
        DiscoveredServer {
            address: a.into(),
            source: s,
            detail: None,
        }
    }
    fn srv() -> Vec<DiscoveredServer> {
        vec![
            d("s1.x:1", ServerSource::DnsSrv),
            d("s2.x:1", ServerSource::DnsSrv),
        ]
    }
    fn ann() -> Vec<DiscoveredServer> {
        vec![d("a1.x:1", ServerSource::DisplayAnnouncement)]
    }
    fn def() -> Vec<DiscoveredServer> {
        vec![d("def.x:443", ServerSource::DefaultList)]
    }
    fn pol(f: impl FnOnce(&mut ServerPolicy)) -> PolicyState {
        let mut p = ServerPolicy {
            servers: vec!["m1.x:1".into(), "m2.x:1".into()],
            ..ServerPolicy::default()
        };
        f(&mut p);
        PolicyState::Active(p)
    }
    fn addrs(m: &Merged) -> Vec<&str> {
        m.servers.iter().map(|s| s.address.as_str()).collect()
    }

    #[test]
    fn no_policy_order_is_srv_announcement_default() {
        let m = merge(&PolicyState::Absent, &srv(), &ann(), &def());
        assert_eq!(addrs(&m), ["s1.x:1", "s2.x:1", "a1.x:1", "def.x:443"]);
        let sources: Vec<_> = m.servers.iter().map(|s| s.source).collect();
        assert_eq!(
            sources,
            [
                ServerSource::DnsSrv,
                ServerSource::DnsSrv,
                ServerSource::DisplayAnnouncement,
                ServerSource::DefaultList
            ]
        );
        assert!(!m.locked && m.warnings.is_empty() && m.mode_hint.is_none());
    }

    #[test]
    fn policy_servers_come_first() {
        let m = merge(&pol(|_| {}), &srv(), &ann(), &def());
        assert_eq!(
            addrs(&m),
            [
                "m1.x:1",
                "m2.x:1",
                "s1.x:1",
                "s2.x:1",
                "a1.x:1",
                "def.x:443"
            ]
        );
        assert_eq!(m.servers[0].source, ServerSource::Mdm);
        assert!(!m.locked);
    }

    #[test]
    fn each_source_alone() {
        let n = PolicyState::Absent;
        assert_eq!(addrs(&merge(&n, &srv(), &[], &[])), ["s1.x:1", "s2.x:1"]);
        assert_eq!(addrs(&merge(&n, &[], &ann(), &[])), ["a1.x:1"]);
        assert_eq!(addrs(&merge(&n, &[], &[], &def())), ["def.x:443"]);
        assert!(merge(&n, &[], &[], &[]).servers.is_empty());
        assert_eq!(
            addrs(&merge(&pol(|_| {}), &[], &[], &[])),
            ["m1.x:1", "m2.x:1"]
        );
    }

    #[test]
    fn forbid_default_list() {
        let m = merge(
            &pol(|p| p.forbid_default_list = true),
            &srv(),
            &ann(),
            &def(),
        );
        assert!(!addrs(&m).contains(&"def.x:443"));
        assert!(addrs(&m).contains(&"a1.x:1"));
        assert!(!m.locked);
    }

    #[test]
    fn forbid_announcement() {
        let m = merge(
            &pol(|p| p.forbid_announcement = true),
            &srv(),
            &ann(),
            &def(),
        );
        assert!(!addrs(&m).contains(&"a1.x:1"));
        assert!(addrs(&m).contains(&"def.x:443"));
    }

    #[test]
    fn forbid_user_override_returns_only_policy() {
        let m = merge(
            &pol(|p| p.forbid_user_override = true),
            &srv(),
            &ann(),
            &def(),
        );
        assert_eq!(addrs(&m), ["m1.x:1", "m2.x:1"]);
        assert!(m.locked);
        assert!(m.warnings.is_empty());
    }

    #[test]
    fn forbid_user_override_without_servers_warns() {
        let p = pol(|p| {
            p.servers.clear();
            p.forbid_user_override = true;
        });
        let m = merge(&p, &srv(), &ann(), &def());
        assert!(m.servers.is_empty() && m.locked);
        assert_eq!(m.warnings.len(), 1);
    }

    #[test]
    fn all_flags_together() {
        let p = pol(|p| {
            p.forbid_default_list = true;
            p.forbid_announcement = true;
        });
        assert_eq!(
            addrs(&merge(&p, &srv(), &ann(), &def())),
            ["m1.x:1", "m2.x:1", "s1.x:1", "s2.x:1"]
        );
    }

    #[test]
    fn invalid_policy_fails_closed() {
        let m = merge(&PolicyState::Invalid("x".into()), &srv(), &ann(), &def());
        assert!(m.servers.is_empty() && m.locked);
        assert_eq!(m.warnings.len(), 1);
    }

    #[test]
    fn duplicates_keep_the_best_source_case_insensitively() {
        let srv = vec![
            d("M1.X:1", ServerSource::DnsSrv),
            d("s1.x:1", ServerSource::DnsSrv),
        ];
        let ann = vec![d("S1.x:1", ServerSource::DisplayAnnouncement)];
        let def = vec![
            d("a1.x:1", ServerSource::DefaultList),
            d("a1.x:1", ServerSource::DefaultList),
        ];
        let m = merge(&pol(|_| {}), &srv, &ann, &def);
        assert_eq!(addrs(&m), ["m1.x:1", "m2.x:1", "s1.x:1", "a1.x:1"]);
        assert_eq!(m.servers[0].source, ServerSource::Mdm);
        assert_eq!(m.servers[2].source, ServerSource::DnsSrv);
    }

    #[test]
    fn mode_hint_is_passed_on_even_when_locked() {
        let m = merge(
            &pol(|p| {
                p.mode = Some(ModeHint::Open);
                p.forbid_user_override = true;
            }),
            &[],
            &[],
            &[],
        );
        assert_eq!(m.mode_hint, Some(ModeHint::Open));
    }
}
