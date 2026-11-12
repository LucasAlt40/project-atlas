//! The system's name lookup, behind `mcp::destination::HostResolver` (ADR 0046).

use std::net::{IpAddr, ToSocketAddrs};

use crate::application::mcp::destination::HostResolver;

pub struct SystemHostResolver;

impl HostResolver for SystemHostResolver {
    fn resolve(&self, host: &str, port: u16) -> Result<Vec<IpAddr>, String> {
        let mut found: Vec<IpAddr> = (host, port)
            .to_socket_addrs()
            .map_err(|error| error.to_string())?
            .map(|address| address.ip())
            .collect();
        found.sort();
        found.dedup();
        Ok(found)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn localhost_is_looked_up_without_the_network() {
        let found = SystemHostResolver.resolve("localhost", 443).unwrap();
        assert!(found.iter().all(IpAddr::is_loopback), "{found:?}");
        assert_ne!(found.len(), 0);
    }
}
