//! Which address to offer the phone. The QR is made from an address an interface really has, never
//! from a guess; with several, one is recommended and the others are offered.

use std::net::{IpAddr, Ipv4Addr};

use serde::Serialize;

/// A network interface address as the operating system reports it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InterfaceAddress {
    pub interface: String,
    pub ip: IpAddr,
}

/// Port: what the machine's interfaces are. The system adapter lives in `infrastructure`.
pub trait NetworkInterfaces: Send + Sync {
    fn addresses(&self) -> Vec<InterfaceAddress>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AddressKind {
    /// A private LAN address (Wi-Fi, Ethernet): what a phone on the same network can reach.
    Lan,
    /// A VPN-style address (100.64.0.0/10, e.g. Tailscale): reachable through the VPN, encrypted by it.
    Vpn,
    /// This computer only. Offered for testing in a browser here; a phone cannot reach it.
    Loopback,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Candidate {
    pub address: String,
    pub interface: String,
    pub kind: AddressKind,
    /// The one used unless the person picks another.
    pub recommended: bool,
}

/// Interface names that are not a network a phone is on (containers, VMs, tunnels, Apple's
/// peer-to-peer).
fn looks_virtual(name: &str) -> bool {
    const PREFIXES: &[&str] = &[
        "docker", "veth", "br-", "bridge", "vmnet", "vboxnet", "virbr", "utun", "awdl", "llw",
        "tap", "tun",
    ];
    const CONTAINS: &[&str] = &["vethernet", "hyper-v", "wsl", "vmware", "virtualbox"];
    let lower = name.to_ascii_lowercase();
    PREFIXES.iter().any(|p| lower.starts_with(p)) || CONTAINS.iter().any(|c| lower.contains(c))
}

fn kind_of(ip: Ipv4Addr) -> Option<AddressKind> {
    let [a, b, ..] = ip.octets();
    if ip.is_loopback() {
        Some(AddressKind::Loopback)
    } else if a == 10 || (a == 172 && (16..=31).contains(&b)) || (a == 192 && b == 168) {
        Some(AddressKind::Lan)
    } else if a == 100 && (64..=127).contains(&b) {
        Some(AddressKind::Vpn)
    } else {
        // Public, link-local (169.254), multicast…: never offered. A public address would put the
        // panel on the internet's side of the machine.
        None
    }
}

/// The addresses worth offering, best first. IPv4 only: a phone typing or scanning an IPv6 literal
/// is the exception, and link-local IPv6 needs a zone id no QR can carry.
pub fn candidates(interfaces: &dyn NetworkInterfaces) -> Vec<Candidate> {
    let mut found: Vec<(bool, Candidate)> = interfaces
        .addresses()
        .into_iter()
        .filter_map(|entry| {
            let IpAddr::V4(ip) = entry.ip else {
                return None;
            };
            let kind = kind_of(ip)?;
            let virtual_name = kind != AddressKind::Loopback && looks_virtual(&entry.interface);
            Some((
                virtual_name,
                Candidate {
                    address: ip.to_string(),
                    interface: entry.interface,
                    kind,
                    recommended: false,
                },
            ))
        })
        .collect();
    let rank = |(is_virtual, c): &(bool, Candidate)| {
        (
            match c.kind {
                AddressKind::Lan => 0,
                AddressKind::Vpn => 1,
                AddressKind::Loopback => 2,
            },
            *is_virtual,
        )
    };
    found.sort_by(|a, b| rank(a).cmp(&rank(b)).then(a.1.address.cmp(&b.1.address)));
    found.dedup_by(|a, b| a.1.address == b.1.address);
    let mut list: Vec<Candidate> = found.into_iter().map(|(_, c)| c).collect();
    if let Some(first) = list.first_mut() {
        if first.kind != AddressKind::Loopback {
            first.recommended = true;
        }
    }
    list
}

#[cfg(test)]
pub mod fake {
    use super::*;

    pub struct Fixed(pub Vec<(&'static str, &'static str)>);

    impl NetworkInterfaces for Fixed {
        fn addresses(&self) -> Vec<InterfaceAddress> {
            self.0
                .iter()
                .map(|(interface, ip)| InterfaceAddress {
                    interface: (*interface).to_owned(),
                    ip: ip.parse().unwrap(),
                })
                .collect()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::fake::Fixed;
    use super::*;

    #[test]
    fn the_wifi_address_is_recommended_over_virtual_ones() {
        let found = candidates(&Fixed(vec![
            ("lo0", "127.0.0.1"),
            ("docker0", "172.17.0.1"),
            ("en0", "192.168.1.20"),
            ("utun3", "10.8.0.2"),
        ]));
        assert_eq!(found[0].address, "192.168.1.20");
        assert!(found[0].recommended);
        assert_eq!(found.iter().filter(|c| c.recommended).count(), 1);
        assert_eq!(found.last().unwrap().kind, AddressKind::Loopback);
        assert!(
            found.iter().any(|c| c.address == "172.17.0.1"),
            "still offered"
        );
    }

    #[test]
    fn public_and_link_local_addresses_are_never_offered() {
        let found = candidates(&Fixed(vec![
            ("en0", "203.0.113.9"),
            ("en1", "169.254.4.4"),
            ("en2", "8.8.8.8"),
            ("en3", "172.32.0.1"),
        ]));
        assert!(found.is_empty(), "{found:?}");
    }

    #[test]
    fn a_vpn_address_is_offered_after_the_lan_one() {
        let found = candidates(&Fixed(vec![
            ("tailscale0", "100.101.102.103"),
            ("eth0", "10.0.0.5"),
        ]));
        assert_eq!(found[0].address, "10.0.0.5");
        assert_eq!(found[1].kind, AddressKind::Vpn);
    }

    #[test]
    fn loopback_alone_is_offered_but_not_recommended() {
        let found = candidates(&Fixed(vec![("lo", "127.0.0.1")]));
        assert_eq!(found.len(), 1);
        assert!(!found[0].recommended);
    }

    #[test]
    fn ipv6_is_left_out_and_the_same_address_is_listed_once() {
        let found = candidates(&Fixed(vec![
            ("en0", "fe80::1"),
            ("en0", "192.168.0.2"),
            ("en1", "192.168.0.2"),
        ]));
        assert_eq!(found.len(), 1);
    }
}
