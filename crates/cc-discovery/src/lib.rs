// SPDX-License-Identifier: GPL-3.0-or-later
//! Finding the mediation server and (in parallel) receivers in the local network.
//!
//! Server lookup is a ladder, see [`lookup_order`]. Discovery of receivers via mDNS
//! runs IN PARALLEL to this ladder, it is not a step of it.

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

/// The order in which server sources are asked; the first answer wins.
pub fn lookup_order() -> [ServerSource; 4] {
    [
        ServerSource::Mdm,
        ServerSource::DnsSrv,
        ServerSource::DisplayAnnouncement,
        ServerSource::DefaultList,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

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
    }
}
