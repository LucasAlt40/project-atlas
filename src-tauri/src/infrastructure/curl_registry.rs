//! Asks the MCP Registry through the system's `curl` (present on macOS, Linux and Windows 10+):
//! one fixed host, a size cap and a time limit. The webview's CSP forbids the page from reaching
//! the network at all, so this is the only way out and it goes nowhere else.

use std::fmt::Write;
use std::process::{Command, Stdio};

use crate::application::mcp::registry::{RegistryClient, RegistryError};

const HOST: &str = "https://registry.modelcontextprotocol.io/v0/servers";
const MAX_BYTES: &str = "4000000";

pub struct CurlRegistryClient;

fn encode(text: &str) -> String {
    text.bytes()
        .map(|b| {
            if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~') {
                (b as char).to_string()
            } else {
                format!("%{b:02X}")
            }
        })
        .collect()
}

impl RegistryClient for CurlRegistryClient {
    fn search(
        &self,
        query: &str,
        cursor: Option<&str>,
        limit: u32,
    ) -> Result<String, RegistryError> {
        let mut url = format!("{HOST}?version=latest&limit={limit}");
        if !query.trim().is_empty() {
            let _ = write!(url, "&search={}", encode(query.trim()));
        }
        if let Some(cursor) = cursor.filter(|c| !c.is_empty()) {
            let _ = write!(url, "&cursor={}", encode(cursor));
        }
        let output = Command::new("curl")
            .args([
                "--silent",
                "--show-error",
                "--fail",
                "--max-time",
                "45",
                "--max-filesize",
                MAX_BYTES,
                "--proto",
                "=https",
                "--header",
                "Accept: application/json",
                &url,
            ])
            .stdin(Stdio::null())
            .output()
            .map_err(|e| RegistryError(format!("curl could not be started: {e}")))?;
        if !output.status.success() {
            return Err(RegistryError(
                String::from_utf8_lossy(&output.stderr).trim().to_owned(),
            ));
        }
        String::from_utf8(output.stdout)
            .map_err(|_| RegistryError("the answer was not text".to_owned()))
    }
}

#[cfg(test)]
mod tests {
    use super::encode;

    #[test]
    fn a_query_cannot_add_parameters_or_break_the_url() {
        assert_eq!(encode("jira & x=1#"), "jira%20%26%20x%3D1%23");
        assert_eq!(encode("a-b_c.d~e"), "a-b_c.d~e");
    }
}
