//! The official MCP Registry (`registry.modelcontextprotocol.io`) as a place to find servers and to
//! see how to connect them. Atlas asks it questions and never trusts the answers: every name,
//! package and value is checked here, and what becomes a connection goes through the same
//! validation as one typed by hand when it is added.
//!
//! The registry lists two kinds of server. A **package** (`npm`, `PyPI`…) is a program Atlas can start
//! over STDIO: this module turns the ones it understands (`npm` through `npx`, `PyPI` through `uvx`)
//! into an executable, arguments and the variables the server asks for. A **remote** (HTTP/SSE) is
//! shown for what it is and not offered: Atlas has no HTTP transport yet. Images (`OCI`), `NuGet` and
//! MCPB bundles are left out for the same reason, said plainly.

use serde::Serialize;
use serde_json::Value;

/// One page of results.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RegistryPage {
    pub servers: Vec<RegistryServer>,
    pub next_cursor: Option<String>,
}

/// A server as the registry describes it, reduced to what Atlas can use.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RegistryServer {
    /// The registry's reverse-DNS name (`io.github.owner/server`).
    pub name: String,
    pub title: Option<String>,
    pub description: String,
    pub version: String,
    pub repository_url: Option<String>,
    pub website_url: Option<String>,
    /// How Atlas could start it (STDIO). Empty when it only has remotes or packages Atlas does
    /// not run.
    pub installs: Vec<RegistryInstall>,
    /// HTTP/SSE endpoints: listed, not addable.
    pub remotes: Vec<String>,
    /// Packages the registry lists that Atlas leaves out (`oci`, `nuget`, `mcpb`).
    pub unsupported_packages: Vec<String>,
}

/// A program the server runs as.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RegistryInstall {
    /// `npm` or `pypi`.
    pub kind: String,
    pub executable: String,
    /// Arguments that are the same for everyone: `-y`, `name@version`.
    pub args: Vec<String>,
    /// What the server takes after that, some of it for the person to fill in.
    pub inputs: Vec<RegistryInput>,
    pub env: Vec<RegistryEnv>,
}

/// An argument after the package name.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RegistryInput {
    /// `--root` for a named argument; `None` for a positional one.
    pub flag: Option<String>,
    /// A value the registry fixes or suggests; `None` is for the person to give.
    pub value: Option<String>,
    pub description: Option<String>,
    pub required: bool,
}

/// An environment variable the server reads.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RegistryEnv {
    pub name: String,
    pub description: Option<String>,
    pub required: bool,
    /// A credential: it goes to the credential store, never to the configuration.
    pub secret: bool,
    pub default: Option<String>,
}

/// The registry could not be asked or its answer was not usable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegistryError(pub String);

/// The port: one page of search results, as the registry's JSON text.
pub trait RegistryClient: Send + Sync {
    /// # Errors
    ///
    /// The registry could not be reached, answered with an error, or sent too much.
    fn search(
        &self,
        query: &str,
        cursor: Option<&str>,
        limit: u32,
    ) -> Result<String, RegistryError>;
}

/// For a build with no network wired.
pub struct NoRegistry;

impl RegistryClient for NoRegistry {
    fn search(&self, _: &str, _: Option<&str>, _: u32) -> Result<String, RegistryError> {
        Err(RegistryError("no network".to_owned()))
    }
}

fn text(value: Option<&Value>) -> Option<String> {
    value
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
}

/// A link the registry gives, only when it is `https`: it is shown to the person, and a
/// `javascript:` or `file:` link from a third party has no place in the app.
fn https_url(value: Option<&Value>) -> Option<String> {
    text(value).filter(|url| {
        url.get(..8)
            .is_some_and(|scheme| scheme.eq_ignore_ascii_case("https://"))
    })
}

fn flag(value: Option<&Value>) -> bool {
    value.and_then(Value::as_bool).unwrap_or(false)
}

/// `name` or `@scope/name`: letters, digits, `.`, `_`, `-`; never something an option could be
/// mistaken for.
fn safe_package(identifier: &str) -> bool {
    let ok = |part: &str| {
        !part.is_empty()
            && part.starts_with(|c: char| c.is_ascii_alphanumeric())
            && part
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
    };
    match identifier.strip_prefix('@') {
        Some(scoped) => scoped.split_once('/').is_some_and(|(s, n)| ok(s) && ok(n)),
        None => ok(identifier),
    }
}

fn safe_version(version: &str) -> bool {
    !version.is_empty()
        && version.starts_with(|c: char| c.is_ascii_alphanumeric())
        && version
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-' | '+'))
}

fn input_of(argument: &Value) -> Option<RegistryInput> {
    let named = argument.get("type").and_then(Value::as_str) == Some("named");
    let flag_name = if named {
        text(argument.get("name"))
    } else {
        None
    };
    let value = text(argument.get("value")).or_else(|| text(argument.get("default")));
    let required = flag(argument.get("isRequired"));
    // An optional argument with nothing to put after it is not offered.
    if value.is_none() && !required {
        return None;
    }
    Some(RegistryInput {
        flag: flag_name,
        value,
        description: text(argument.get("description")),
        required,
    })
}

fn install_of(package: &Value) -> Result<RegistryInstall, String> {
    let kind = text(package.get("registryType")).unwrap_or_default();
    if package
        .pointer("/transport/type")
        .and_then(Value::as_str)
        .is_some_and(|t| t != "stdio")
    {
        return Err(format!("{kind}/non-stdio"));
    }
    let identifier = text(package.get("identifier")).unwrap_or_default();
    let version = text(package.get("version"));
    let (executable, args) = match kind.as_str() {
        "npm" | "pypi" if !safe_package(&identifier) => return Err(kind),
        "npm" | "pypi" if version.as_deref().is_some_and(|v| !safe_version(v)) => return Err(kind),
        "npm" => {
            let target = version.map_or(identifier.clone(), |v| format!("{identifier}@{v}"));
            ("npx", vec!["-y".to_owned(), target])
        }
        "pypi" => {
            let target = version.map_or(identifier.clone(), |v| format!("{identifier}@{v}"));
            ("uvx", vec![target])
        }
        _ => return Err(kind),
    };
    let inputs = package
        .get("packageArguments")
        .and_then(Value::as_array)
        .map(|list| list.iter().filter_map(input_of).collect())
        .unwrap_or_default();
    let env = package
        .get("environmentVariables")
        .and_then(Value::as_array)
        .map(|list| {
            list.iter()
                .filter_map(|v| {
                    Some(RegistryEnv {
                        name: text(v.get("name"))?,
                        description: text(v.get("description")),
                        required: flag(v.get("isRequired")),
                        secret: flag(v.get("isSecret")),
                        default: text(v.get("default")),
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    Ok(RegistryInstall {
        kind,
        executable: executable.to_owned(),
        args,
        inputs,
        env,
    })
}

fn server_of(entry: &Value) -> Option<RegistryServer> {
    let server = entry.get("server")?;
    let name = text(server.get("name"))?;
    let mut installs = Vec::new();
    let mut unsupported = Vec::new();
    for package in server
        .get("packages")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        match install_of(package) {
            Ok(install) => installs.push(install),
            Err(kind) if !unsupported.contains(&kind) => unsupported.push(kind),
            Err(_) => {}
        }
    }
    let remotes = server
        .get("remotes")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|r| text(r.get("url")))
        .collect();
    Some(RegistryServer {
        title: text(server.get("title")),
        description: text(server.get("description")).unwrap_or_default(),
        version: text(server.get("version")).unwrap_or_default(),
        repository_url: https_url(server.pointer("/repository/url")),
        website_url: https_url(server.get("websiteUrl")),
        name,
        installs,
        remotes,
        unsupported_packages: unsupported,
    })
}

/// Reads a registry answer.
///
/// # Errors
///
/// The text is not the registry's JSON.
pub fn parse_page(body: &str) -> Result<RegistryPage, RegistryError> {
    let value: Value = serde_json::from_str(body)
        .map_err(|_| RegistryError("the answer was not JSON".to_owned()))?;
    let servers = value
        .get("servers")
        .and_then(Value::as_array)
        .ok_or_else(|| RegistryError("the answer has no servers".to_owned()))?
        .iter()
        .filter_map(server_of)
        .collect();
    Ok(RegistryPage {
        servers,
        next_cursor: text(value.pointer("/metadata/nextCursor")),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const BODY: &str = r#"{"servers":[
      {"server":{"name":"io.github.acme/jira","title":"Jira","description":" Issues ","version":"1.2.0",
        "repository":{"url":"https://github.com/acme/jira"},
        "packages":[
          {"registryType":"npm","identifier":"@acme/jira-mcp","version":"1.2.0","transport":{"type":"stdio"},
           "packageArguments":[{"type":"named","name":"--site","isRequired":true,"description":"Site"},
                               {"type":"positional","value":"serve"},
                               {"type":"named","name":"--debug"}],
           "environmentVariables":[{"name":"JIRA_TOKEN","isRequired":true,"isSecret":true},
                                   {"name":"JIRA_URL","default":"https://x.atlassian.net"}]},
          {"registryType":"pypi","identifier":"acme-jira","version":"1.2.0","transport":{"type":"stdio"}},
          {"registryType":"oci","identifier":"ghcr.io/acme/jira:1.2.0","transport":{"type":"stdio"}}]}},
      {"server":{"name":"ai.x/remote","description":"r","version":"1",
        "remotes":[{"type":"streamable-http","url":"https://x/mcp"}]}},
      {"server":{"name":"bad/one","description":"b","version":"1",
        "packages":[{"registryType":"npm","identifier":"--evil","version":"1","transport":{"type":"stdio"}},
                    {"registryType":"npm","identifier":"ok","version":"1; rm","transport":{"type":"stdio"}}]}}
    ],"metadata":{"nextCursor":"abc:1","count":3}}"#;

    #[test]
    fn only_https_links_from_the_registry_are_kept() {
        let link = |url: &str| https_url(Some(&Value::String(url.to_owned())));
        assert_eq!(
            link("https://github.com/acme/jira").as_deref(),
            Some("https://github.com/acme/jira")
        );
        assert_eq!(link("HTTPS://x.dev").as_deref(), Some("HTTPS://x.dev"));
        for refused in [
            "javascript:alert(1)",
            "http://x.dev",
            "file:///etc/passwd",
            "https:",
        ] {
            assert_eq!(link(refused), None, "{refused}");
        }
    }

    #[test]
    fn a_package_becomes_a_command_with_what_the_person_must_fill_in() {
        let page = parse_page(BODY).unwrap();

        let jira = &page.servers[0];
        assert_eq!(jira.title.as_deref(), Some("Jira"));
        assert_eq!(jira.description, "Issues");
        assert_eq!(jira.installs.len(), 2);
        let npm = &jira.installs[0];
        assert_eq!(
            (npm.executable.as_str(), npm.args.as_slice()),
            (
                "npx",
                ["-y".to_owned(), "@acme/jira-mcp@1.2.0".to_owned()].as_slice()
            )
        );
        // The required named argument is asked for; the fixed positional one is kept; the optional
        // flag with nothing after it is not offered.
        assert_eq!(npm.inputs.len(), 2);
        assert_eq!(npm.inputs[0].flag.as_deref(), Some("--site"));
        assert!(npm.inputs[0].value.is_none() && npm.inputs[0].required);
        assert_eq!(npm.inputs[1].value.as_deref(), Some("serve"));
        assert!(npm.env[0].secret && npm.env[0].required);
        assert_eq!(
            npm.env[1].default.as_deref(),
            Some("https://x.atlassian.net")
        );
        let pypi = &jira.installs[1];
        assert_eq!(
            (pypi.executable.as_str(), pypi.args[0].as_str()),
            ("uvx", "acme-jira@1.2.0")
        );
        assert_eq!(jira.unsupported_packages, ["oci"]);
        assert_eq!(page.next_cursor.as_deref(), Some("abc:1"));
    }

    #[test]
    fn a_remote_is_shown_and_not_offered() {
        let remote = &parse_page(BODY).unwrap().servers[1];

        assert_eq!(remote.installs.len(), 0);
        assert_eq!(remote.remotes, ["https://x/mcp"]);
    }

    #[test]
    fn a_package_name_or_version_that_could_be_an_option_or_a_command_is_left_out() {
        let bad = &parse_page(BODY).unwrap().servers[2];

        assert_eq!(bad.installs.len(), 0);
        assert_eq!(bad.unsupported_packages, ["npm"]);
    }

    #[test]
    fn something_that_is_not_the_registry_is_an_error() {
        assert!(parse_page("<html>").is_err());
        assert!(parse_page("{}").is_err());
    }
}
