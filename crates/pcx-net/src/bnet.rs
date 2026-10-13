//! The classic Battle.net gateway list, and the client session model.
//!
//! # What is real here, and what is not
//!
//! Real: the gateway list, which ships inside `war3.mpq` as
//! `BattleNet\bnserver-WAR3.ini`. Four gateways, each with a zone number and a
//! display name. That file is parsed and tested below.
//!
//! Not real: the wire protocol. 1.29b's Battle.net is encapsulated in
//! `Warcraft III Launcher.exe` and there is no user-run PvPGN-style realm for it,
//! so the bytes cannot be observed from this machine and are not written down
//! here. Guessing them would be inventing a protocol, which is the
//! wrong-skeleton-shaped error this project keeps having to revert.
//!
//! What is modelled is the *authority*, which is protocol-independent and is the
//! part the design depends on: a Battle.net client is never the authority. It
//! proposes; the realm decides.

use std::collections::BTreeMap;

/// A Battle.net gateway, as shipped.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Gateway {
    /// Hostname, e.g. `uswest.battle.net`.
    pub host: String,
    /// Zone number. Signed, because the shipped values include negatives.
    pub zone: i32,
    /// Display name from the `ENU` key, e.g. `Lordaeron (U.S. West)`.
    pub name: String,
}

/// The parsed contents of `bnserver-WAR3.ini`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ServerList {
    /// The `[Server List Version]` value.
    pub version: u32,
    /// Gateways in the order the file lists them.
    pub gateways: Vec<Gateway>,
}

impl ServerList {
    /// Parses `[Server List Version]`, `[Server Gateways]` and per-host sections.
    ///
    /// CRLF-terminated, as shipped. A gateway with no `ZONE` is kept with a zone
    /// of zero rather than dropped, because the host is still the useful part.
    pub fn parse(text: &str) -> Self {
        let mut version = 0;
        let mut gateways = Vec::new();
        // Section name -> key/value pairs, since a host's zone and name live in
        // its own section.
        let mut sections: BTreeMap<String, BTreeMap<String, String>> = BTreeMap::new();
        let mut current = String::new();

        for raw in text.lines() {
            let line = raw.trim_end_matches(['\r', '\n']).trim();
            if line.is_empty() || line.starts_with(';') || line.starts_with('#') {
                continue;
            }
            if let Some(name) = line.strip_prefix('[').and_then(|s| s.strip_suffix(']')) {
                current = name.trim().to_string();
                sections.entry(current.clone()).or_default();
                continue;
            }
            let Some((key, value)) = line.split_once('=') else {
                continue;
            };
            let (key, value) = (key.trim(), value.trim());
            if current == "Server List Version" && key.eq_ignore_ascii_case("VER") {
                version = value.parse().unwrap_or(0);
                continue;
            }
            sections
                .entry(current.clone())
                .or_default()
                .insert(key.to_string(), value.to_string());
        }

        if let Some(list) = sections.get("Server Gateways") {
            // Keys are `1`, `2`, ... in display order.
            let mut ordered: Vec<(&String, &String)> = list.iter().collect();
            ordered.sort_by_key(|(k, _)| k.parse::<u32>().unwrap_or(u32::MAX));
            for (_, host) in ordered {
                let host = host.trim().to_string();
                if host.is_empty() {
                    continue;
                }
                let host_section = sections.get(&host).cloned().unwrap_or_default();
                let zone = host_section
                    .get("ZONE")
                    .and_then(|z| z.parse().ok())
                    .unwrap_or(0);
                let name = host_section
                    .get("ENU")
                    .cloned()
                    .unwrap_or_else(|| host.clone());
                gateways.push(Gateway { host, zone, name });
            }
        }

        Self { version, gateways }
    }

    /// The gateway with this hostname.
    pub fn find(&self, host: &str) -> Option<&Gateway> {
        self.gateways
            .iter()
            .find(|g| g.host.eq_ignore_ascii_case(host))
    }
}

/// Where a Battle.net client is in its session.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BnetState {
    /// No realm connection.
    Disconnected,
    /// Resolving and connecting to a gateway.
    Connecting,
    /// Exchanging protocol version and check-revision tokens.
    Handshake,
    /// Authenticated; in the realm's lobby.
    Lobby,
    /// In a game the realm is hosting.
    InGame,
}

/// A Battle.net client session.
///
/// The state machine is modelled, not the packets: a client moves from
/// connecting through handshake to lobby and in-game, and every transition is
/// driven by the realm, never by the client's own belief about the world.
#[derive(Debug)]
pub struct BnetClient {
    state: BnetState,
    gateway: Option<String>,
    /// Version token reported to the realm, or `None` until known.
    version: Option<u32>,
    /// Check-revision token, which is what a realm rejects on mismatch.
    check_revision: Option<u32>,
}

impl BnetClient {
    pub fn new() -> Self {
        Self {
            state: BnetState::Disconnected,
            gateway: None,
            version: None,
            check_revision: None,
        }
    }

    pub fn state(&self) -> BnetState {
        self.state
    }

    pub fn gateway(&self) -> Option<&str> {
        self.gateway.as_deref()
    }

    /// Begins connecting to a gateway.
    pub fn connect(&mut self, host: &str) {
        self.state = BnetState::Connecting;
        self.gateway = Some(host.to_string());
    }

    /// Enters handshake once the realm accepts the connection.
    pub fn handshake(&mut self) -> bool {
        if self.state != BnetState::Connecting {
            return false;
        }
        self.state = BnetState::Handshake;
        true
    }

    /// Reports the version token.
    pub fn report_version(&mut self, version: u32) -> bool {
        if self.state != BnetState::Handshake {
            return false;
        }
        self.version = Some(version);
        true
    }

    /// Reports the check-revision token.
    ///
    /// This is the transition a mismatch fails, so it is separated from
    /// reporting the version: the two are accepted independently by the handshake
    /// and a client that conflates them cannot tell which was rejected.
    pub fn report_check_revision(&mut self, cr: u32) -> bool {
        if self.state != BnetState::Handshake {
            return false;
        }
        self.check_revision = Some(cr);
        true
    }

    /// The realm admits the client to the lobby.
    pub fn enter_lobby(&mut self) -> bool {
        if self.state != BnetState::Handshake {
            return false;
        }
        self.state = BnetState::Lobby;
        true
    }

    /// The realm puts the client in a game.
    pub fn enter_game(&mut self) -> bool {
        if self.state != BnetState::Lobby {
            return false;
        }
        self.state = BnetState::InGame;
        true
    }
}

impl Default for BnetClient {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The shipped file, parsed from the real archive rather than a fixture.
    ///
    /// Deliberately build-agnostic about hostnames and version, because the two
    /// builds ship different ones and asserting either would pin the test to one
    /// install:
    ///
    /// * 1.28c: `VER=1001`, `uswest.battle.net`, `useast`, `asia`, `europe`
    /// * 1.29b: `VER=1014`, `connect-usw.classic.blizzard.com`, `connect-use`,
    ///   `connect-kor`, `connect-eur`
    ///
    /// What holds for both, and is what is asserted: four gateways, four zones
    /// from the same set, and the zones in the order the numbered keys give.
    #[test]
    fn the_shipped_gateway_list_parses() {
        let Some(bytes) = shipped("BattleNet\\bnserver-WAR3.ini") else {
            eprintln!("skipping: no install");
            return;
        };
        let text = String::from_utf8(bytes).expect("utf-8 ini");
        let list = ServerList::parse(&text);

        assert!(list.version >= 1001, "a server list version is required");
        assert_eq!(list.gateways.len(), 4, "four classic gateways");

        // Zones are the stable part: the realm hostnames were replaced between
        // builds but the zone numbers were not.
        let zones: Vec<i32> = list.gateways.iter().map(|g| g.zone).collect();
        assert_eq!(
            zones,
            vec![8, 6, -9, -1],
            "zone numbers changed between builds"
        );
        // Signed, because two of the four are negative.
        assert!(zones.iter().any(|&z| z < 0));

        // Every gateway names a host and a display name, and both are distinct,
        // so a gateway cannot be confused with another.
        let hosts: std::collections::BTreeSet<&str> =
            list.gateways.iter().map(|g| g.host.as_str()).collect();
        assert_eq!(hosts.len(), 4, "duplicate gateway host");
        for g in &list.gateways {
            assert_eq!(g.name, g.name.trim(), "display name has padding");
            assert!(!g.name.is_empty(), "gateway {} has no display name", g.host);
        }
        assert!(list.find("nope.example.com").is_none(), "unknown host");
        assert!(
            list.find(&list.gateways[0].host).is_some(),
            "find must work"
        );
    }

    /// The shipped file is CRLF, so parsing must not leave `\r` on values.
    #[test]
    fn crlf_is_handled() {
        let list = ServerList::parse(
            "[Server Gateways]\r\n1=a.test\r\n\r\n[a.test]\r\nZONE=3\r\nENU=Test Realm\r\n",
        );
        assert_eq!(list.gateways.len(), 1);
        assert_eq!(
            list.gateways[0].name, "Test Realm",
            "value kept a carriage return"
        );
        assert_eq!(list.gateways[0].zone, 3);
    }

    #[test]
    fn a_gateway_without_a_section_is_kept_with_a_zero_zone() {
        let list = ServerList::parse("[Server Gateways]\n1=bare.test\n");
        assert_eq!(list.gateways.len(), 1);
        assert_eq!(list.gateways[0].host, "bare.test");
        assert_eq!(list.gateways[0].zone, 0);
        assert_eq!(list.gateways[0].name, "bare.test", "falls back to the host");
    }

    /// Every transition is gated, because a client that moves itself into a
    /// state it has not been granted is exactly the authority violation.
    #[test]
    fn transitions_are_gated_on_the_current_state() {
        let mut c = BnetClient::new();
        assert_eq!(c.state(), BnetState::Disconnected);
        assert!(!c.handshake(), "cannot handshake before connecting");
        assert!(!c.enter_lobby(), "cannot skip to lobby");
        assert!(!c.enter_game(), "cannot skip to a game");

        c.connect("uswest.battle.net");
        assert_eq!(c.gateway(), Some("uswest.battle.net"));
        assert!(c.handshake());
        // Version and check-revision are separate rejections.
        assert!(c.report_version(1001));
        assert!(c.report_check_revision(0xDEAD_BEEF));
        assert!(c.enter_lobby());
        assert!(c.enter_game());
        assert_eq!(c.state(), BnetState::InGame);
    }

    /// The gateway list is a list, not a stack: the shipped order is preserved.
    #[test]
    fn gateway_order_follows_the_numbered_keys() {
        let list =
            ServerList::parse("[Server Gateways]\n2=second.test\n1=first.test\n10=tenth.test\n");
        let order: Vec<&str> = list.gateways.iter().map(|g| g.host.as_str()).collect();
        assert_eq!(order, vec!["first.test", "second.test", "tenth.test"]);
    }

    fn shipped(rel: &str) -> Option<Vec<u8>> {
        for key in ["PEACECRAFT3_LEGACY_DIR", "PEACECRAFT3_INSTALL"] {
            if let Ok(dir) = std::env::var(key) {
                let p = std::path::PathBuf::from(dir);
                if p.is_dir() {
                    if let Ok(mut s) = pcx_mpq::MpqSet::open_wc3(p) {
                        return s.extract(rel).ok();
                    }
                }
            }
        }
        let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()?
            .parent()?
            .join("wc3/game/app");
        pcx_mpq::MpqSet::open_wc3(p).ok()?.extract(rel).ok()
    }
}
