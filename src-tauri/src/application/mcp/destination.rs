//! Where a remote MCP server may be (ADR 0046): the SSRF layer. The runtime's CLI connects to the
//! server itself, so Atlas cannot see each request; what it can do is refuse to hand a runtime a
//! destination it must not reach, when the connection is saved and again right before every step
//! that is given it.
//!
//! The rule, after the name is looked up (every address it resolves to counts):
//!
//! | Address                                          | Allowed                                   |
//! | ------------------------------------------------ | ----------------------------------------- |
//! | public                                           | yes, over `https`                         |
//! | loopback, private, CGNAT, unique-local (IPv6)    | only when the person set `allow_private`  |
//! | link-local (`169.254/16`, `fe80::/10`), metadata | never: that is where cloud metadata lives |
//! | unspecified, multicast, broadcast, documentation | never                                     |
//!
//! Plain `http` is accepted only for a destination that is entirely on the person's own machine or
//! network (`allow_private`): never in clear text over the internet.
//!
//! What this cannot do, and the ADR says so: a name that resolves to a public address now and to a
//! private one when the CLI connects (DNS rebinding) is not caught, and a redirect the CLI follows
//! is not seen.

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

/// Port: looking a host name up. The system implementation is in `infrastructure/`.
pub trait HostResolver: Send + Sync {
    /// Every address `host` resolves to.
    ///
    /// # Errors
    ///
    /// The name could not be looked up.
    fn resolve(&self, host: &str, port: u16) -> Result<Vec<IpAddr>, String>;
}

/// Why a destination is refused: a code the UI words.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Blocked {
    /// Not a URL Atlas can read, or no host in it.
    Malformed,
    /// The name could not be looked up (offline, or it does not exist).
    Unresolved,
    /// Loopback, a private network, CGNAT or unique-local, and the person did not allow it.
    Private,
    /// Link-local or a known metadata endpoint: never.
    Metadata,
    /// An address no server is at (unspecified, multicast, broadcast, documentation ranges).
    Reserved,
    /// Plain `http` to a destination that is not entirely private.
    PlainText,
}

impl Blocked {
    pub fn code(self) -> &'static str {
        match self {
            Self::Malformed => "malformed",
            Self::Unresolved => "unresolved",
            Self::Private => "private",
            Self::Metadata => "metadata",
            Self::Reserved => "reserved",
            Self::PlainText => "plain_text",
        }
    }
}

/// Host names that are metadata endpoints whatever they resolve to.
const METADATA_NAMES: &[&str] = &[
    "metadata",
    "metadata.google.internal",
    "metadata.goog",
    "instance-data",
    "instance-data.ec2.internal",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Public,
    Private,
    Metadata,
    Reserved,
}

fn kind_v4(ip: Ipv4Addr) -> Kind {
    let [a, b, ..] = ip.octets();
    if ip.is_link_local() {
        return Kind::Metadata;
    }
    if ip.is_unspecified()
        || ip.is_multicast()
        || ip.is_broadcast()
        || ip.is_documentation()
        || a == 0
        || a >= 240
    {
        return Kind::Reserved;
    }
    // 100.64.0.0/10 (CGNAT) and 198.18.0.0/15 (benchmarking) are not on the internet either.
    let cgnat = a == 100 && (64..128).contains(&b);
    let benchmarking = a == 198 && (b == 18 || b == 19);
    if ip.is_loopback() || ip.is_private() || cgnat || benchmarking {
        return Kind::Private;
    }
    Kind::Public
}

fn kind_v6(ip: Ipv6Addr) -> Kind {
    if let Some(v4) = ip.to_ipv4_mapped() {
        return kind_v4(v4);
    }
    let first = ip.segments()[0];
    // fd00:ec2::254, the IPv6 metadata endpoint, is inside fc00::/7; it is named first.
    if ip == Ipv6Addr::new(0xfd00, 0x0ec2, 0, 0, 0, 0, 0, 0x0254) || (first & 0xffc0) == 0xfe80 {
        return Kind::Metadata;
    }
    if ip.is_unspecified() || ip.is_multicast() || first == 0x2001 && ip.segments()[1] == 0x0db8 {
        return Kind::Reserved;
    }
    if ip.is_loopback() || (first & 0xfe00) == 0xfc00 {
        return Kind::Private;
    }
    Kind::Public
}

fn kind(ip: IpAddr) -> Kind {
    match ip {
        IpAddr::V4(v4) => kind_v4(v4),
        IpAddr::V6(v6) => kind_v6(v6),
    }
}

/// Whether a runtime may be handed `url`.
///
/// # Errors
///
/// Why not ([`Blocked`]).
pub fn check(url: &str, allow_private: bool, resolver: &dyn HostResolver) -> Result<(), Blocked> {
    let parsed = url::Url::parse(url).map_err(|_| Blocked::Malformed)?;
    let plain_text = match parsed.scheme() {
        "https" => false,
        "http" => true,
        _ => return Err(Blocked::Malformed),
    };
    let port = parsed.port_or_known_default().ok_or(Blocked::Malformed)?;
    let addresses: Vec<IpAddr> = match parsed.host().ok_or(Blocked::Malformed)? {
        url::Host::Ipv4(ip) => vec![IpAddr::V4(ip)],
        url::Host::Ipv6(ip) => vec![IpAddr::V6(ip)],
        url::Host::Domain(name) => {
            let name = name.trim_end_matches('.').to_ascii_lowercase();
            if METADATA_NAMES.contains(&name.as_str()) {
                return Err(Blocked::Metadata);
            }
            let found = resolver
                .resolve(&name, port)
                .map_err(|_| Blocked::Unresolved)?;
            if found.is_empty() {
                return Err(Blocked::Unresolved);
            }
            found
        }
    };
    let kinds: Vec<Kind> = addresses.into_iter().map(kind).collect();
    if kinds.contains(&Kind::Metadata) {
        return Err(Blocked::Metadata);
    }
    if kinds.contains(&Kind::Reserved) {
        return Err(Blocked::Reserved);
    }
    let any_private = kinds.contains(&Kind::Private);
    if any_private && !allow_private {
        return Err(Blocked::Private);
    }
    if plain_text && (!allow_private || kinds.contains(&Kind::Public)) {
        return Err(Blocked::PlainText);
    }
    Ok(())
}

#[cfg(test)]
pub mod fake {
    use std::collections::HashMap;
    use std::net::IpAddr;

    /// Names resolve to what the test says; anything else does not resolve.
    #[derive(Default)]
    pub struct FakeResolver(pub HashMap<String, Vec<IpAddr>>);

    impl FakeResolver {
        pub fn with(entries: &[(&str, &[&str])]) -> Self {
            Self(
                entries
                    .iter()
                    .map(|(name, ips)| {
                        (
                            (*name).to_owned(),
                            ips.iter().map(|ip| ip.parse().unwrap()).collect(),
                        )
                    })
                    .collect(),
            )
        }
    }

    impl super::HostResolver for FakeResolver {
        fn resolve(&self, host: &str, _port: u16) -> Result<Vec<IpAddr>, String> {
            self.0
                .get(host)
                .cloned()
                .ok_or_else(|| "not found".to_owned())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::fake::FakeResolver;
    use super::*;

    fn dns() -> FakeResolver {
        FakeResolver::with(&[
            ("mcp.example.com", &["93.184.216.34"]),
            ("intranet.local", &["10.0.0.7"]),
            ("mixed.example.com", &["93.184.216.34", "192.168.1.2"]),
            ("rebind-to-metadata.example.com", &["169.254.169.254"]),
            ("v6.example.com", &["2606:2800:220:1:248:1893:25c8:1946"]),
            ("mapped.example.com", &["::ffff:127.0.0.1"]),
        ])
    }

    #[test]
    fn a_public_https_server_is_allowed_and_its_name_is_looked_up() {
        assert_eq!(check("https://mcp.example.com/mcp", false, &dns()), Ok(()));
        assert_eq!(check("https://v6.example.com/mcp", false, &dns()), Ok(()));
        assert_eq!(
            check("https://93.184.216.34:8443/mcp", false, &dns()),
            Ok(())
        );
    }

    #[test]
    fn loopback_and_private_networks_need_the_persons_permission() {
        for url in [
            "https://127.0.0.1/mcp",
            "https://[::1]/mcp",
            "https://intranet.local/mcp",
            "https://10.1.2.3/mcp",
            "https://172.16.0.1/mcp",
            "https://100.64.0.1/mcp",
            "https://[fd12::1]/mcp",
            "https://mapped.example.com/mcp",
            // One private address among public ones is enough.
            "https://mixed.example.com/mcp",
        ] {
            assert_eq!(check(url, false, &dns()), Err(Blocked::Private), "{url}");
            assert_eq!(check(url, true, &dns()), Ok(()), "{url}");
        }
    }

    #[test]
    fn metadata_endpoints_are_never_reachable_whatever_the_person_allows() {
        for url in [
            "http://169.254.169.254/latest/meta-data",
            "https://169.254.1.1/",
            "https://[fe80::1]/",
            "https://[fd00:ec2::254]/",
            "https://metadata.google.internal/computeMetadata/v1/",
            "https://METADATA.google.internal./",
            "https://rebind-to-metadata.example.com/",
            "https://[::ffff:169.254.169.254]/",
        ] {
            for allow in [false, true] {
                assert_eq!(check(url, allow, &dns()), Err(Blocked::Metadata), "{url}");
            }
        }
    }

    #[test]
    fn clear_text_is_only_for_the_persons_own_network() {
        assert_eq!(
            check("http://mcp.example.com/mcp", false, &dns()),
            Err(Blocked::PlainText)
        );
        assert_eq!(
            check("http://mcp.example.com/mcp", true, &dns()),
            Err(Blocked::PlainText)
        );
        assert_eq!(
            check("http://127.0.0.1:3000/mcp", false, &dns()),
            Err(Blocked::Private)
        );
        assert_eq!(check("http://127.0.0.1:3000/mcp", true, &dns()), Ok(()));
        // A name that is partly public is not "the person's own network".
        assert_eq!(
            check("http://mixed.example.com/mcp", true, &dns()),
            Err(Blocked::PlainText)
        );
    }

    #[test]
    fn what_cannot_be_read_or_reached_is_refused() {
        for (url, why) in [
            ("not a url", Blocked::Malformed),
            ("ftp://mcp.example.com/", Blocked::Malformed),
            ("file:///etc/passwd", Blocked::Malformed),
            ("https://nowhere.example.com/", Blocked::Unresolved),
            ("https://0.0.0.0/", Blocked::Reserved),
            ("https://224.0.0.1/", Blocked::Reserved),
            ("https://255.255.255.255/", Blocked::Reserved),
            ("https://192.0.2.1/", Blocked::Reserved),
            ("https://[::]/", Blocked::Reserved),
        ] {
            assert_eq!(check(url, true, &dns()), Err(why), "{url}");
        }
    }
}
