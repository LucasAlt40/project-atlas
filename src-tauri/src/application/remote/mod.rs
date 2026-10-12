//! Mobile access (ADR 0034): watching and answering workflow runs from a phone on the local
//! network.
//!
//! This module holds the rules; `infrastructure::remote_server` is the HTTP adapter around them
//! and `commands::remote` is how the desktop switches it on. It adds no way to run anything: it
//! reads what the Orchestrator keeps and hands answers back to it.

pub mod gateway;
pub mod hub;
pub mod limits;
pub mod network;
pub mod policy;
pub mod sessions;
pub mod tokens;
pub mod view;

use std::net::IpAddr;
use std::sync::{Arc, Mutex};

use serde::Serialize;

use self::gateway::RemoteGateway;
use self::hub::EventHub;
use self::limits::RateLimiter;
use self::network::{Candidate, NetworkInterfaces};
use self::sessions::{Device, PairingStore, SessionStore};
use crate::application::support::{now_ms, LockExt};

/// Port: the files of the mobile page. The adapter reads them from the app's embedded frontend.
pub trait AssetSource: Send + Sync {
    /// The bytes and media type of a file, by path without a leading slash (`mobile.html`,
    /// `assets/mobile-3f2a.js`).
    fn get(&self, path: &str) -> Option<Asset>;
}

pub struct Asset {
    pub bytes: Vec<u8>,
    pub content_type: String,
}

/// The port the server may be asked for first, by the preferred range, then the next ones.
pub const PORTS: std::ops::RangeInclusive<u16> = 47615..=47634;

/// Why the server is not running.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RemoteFailure {
    /// The computer has no address a phone could reach.
    NoNetwork,
    /// The chosen address is not one of the computer's.
    AddressUnavailable,
    /// Every port of the range is taken by another program.
    PortUnavailable,
    /// The system refused to listen for another reason.
    BindFailed,
    AlreadyRunning,
    NotRunning,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RemoteState {
    Disabled,
    Starting,
    Active,
    Error,
}

enum Lifecycle {
    Disabled,
    Starting,
    Active {
        address: IpAddr,
        port: u16,
        /// Ends the listener and every connection.
        stop: Box<dyn FnOnce() + Send>,
    },
    Error(RemoteFailure),
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PairingView {
    /// As a person types it (`ABCD-EFGH-…`).
    pub code: String,
    /// What the QR opens.
    pub url: String,
    pub qr_svg: String,
    pub expires_at: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteStatus {
    pub state: RemoteState,
    pub error: Option<RemoteFailure>,
    /// Where the phone connects (no code in it).
    pub url: Option<String>,
    pub address: Option<String>,
    pub port: Option<u16>,
    /// The address the server is on is no longer one of the computer's (the network changed).
    pub address_lost: bool,
    pub candidates: Vec<Candidate>,
    pub pairing: Option<PairingView>,
    pub devices: Vec<Device>,
    /// Questions waiting that the phone can answer.
    pub waiting: u32,
    pub server_time: u64,
}

pub struct RemoteAccess {
    pub sessions: SessionStore,
    pub pairing: PairingStore,
    pub hub: Arc<EventHub>,
    pub gateway: Arc<RemoteGateway>,
    /// Writes (answers, controls) per device.
    pub writes: RateLimiter,
    /// Pairing attempts per address.
    pub pair_attempts: RateLimiter,
    interfaces: Arc<dyn NetworkInterfaces>,
    lifecycle: Mutex<Lifecycle>,
}

impl RemoteAccess {
    pub fn new(gateway: Arc<RemoteGateway>, interfaces: Arc<dyn NetworkInterfaces>) -> Self {
        Self {
            sessions: SessionStore::default(),
            pairing: PairingStore::default(),
            hub: Arc::new(EventHub::new()),
            gateway,
            writes: RateLimiter::new(30, 60_000),
            pair_attempts: RateLimiter::new(10, 60_000),
            interfaces,
            lifecycle: Mutex::new(Lifecycle::Disabled),
        }
    }

    pub fn candidates(&self) -> Vec<Candidate> {
        network::candidates(self.interfaces.as_ref())
    }

    /// Reserves the start and picks the address to listen on: the one asked for if the computer
    /// has it, else the recommended one. Never a free-text address, never `0.0.0.0`.
    ///
    /// # Errors
    ///
    /// [`RemoteFailure`] if it is already running, or no suitable address exists.
    pub fn begin_start(&self, wanted: Option<&str>) -> Result<IpAddr, RemoteFailure> {
        let mut lifecycle = self.lifecycle.lock_or_recover();
        if matches!(*lifecycle, Lifecycle::Starting | Lifecycle::Active { .. }) {
            return Err(RemoteFailure::AlreadyRunning);
        }
        let candidates = self.candidates();
        let chosen = match wanted {
            Some(address) => candidates
                .iter()
                .find(|c| c.address == address)
                .ok_or(RemoteFailure::AddressUnavailable)?,
            None => candidates
                .iter()
                .find(|c| c.recommended)
                .ok_or(RemoteFailure::NoNetwork)?,
        };
        let ip = chosen
            .address
            .parse()
            .map_err(|_| RemoteFailure::AddressUnavailable)?;
        *lifecycle = Lifecycle::Starting;
        Ok(ip)
    }

    /// The listener is up: from now on pairing can happen.
    pub fn mark_active(&self, address: IpAddr, port: u16, stop: Box<dyn FnOnce() + Send>) {
        self.hub.reset();
        self.pairing.issue(now_ms());
        *self.lifecycle.lock_or_recover() = Lifecycle::Active {
            address,
            port,
            stop,
        };
    }

    pub fn mark_failed(&self, failure: RemoteFailure) {
        *self.lifecycle.lock_or_recover() = Lifecycle::Error(failure);
    }

    /// Ends the server, every connection and every session. Runs are not touched.
    pub fn stop(&self) {
        let previous =
            std::mem::replace(&mut *self.lifecycle.lock_or_recover(), Lifecycle::Disabled);
        self.sessions.revoke_all();
        self.pairing.clear();
        if let Lifecycle::Active { stop, .. } = previous {
            self.hub.publish_revoked();
            stop();
        }
    }

    /// A new pairing code; the one before stops working.
    ///
    /// # Errors
    ///
    /// [`RemoteFailure::NotRunning`] when the server is not on.
    pub fn new_pairing_code(&self) -> Result<(), RemoteFailure> {
        if !matches!(*self.lifecycle.lock_or_recover(), Lifecycle::Active { .. }) {
            return Err(RemoteFailure::NotRunning);
        }
        self.pairing.issue(now_ms());
        Ok(())
    }

    pub fn revoke_device(&self, device_id: &str) -> bool {
        let revoked = self.sessions.revoke(device_id);
        if revoked {
            self.hub.publish_revoked();
        }
        revoked
    }

    pub fn revoke_all_devices(&self) -> usize {
        let count = self.sessions.revoke_all();
        // Codes not used yet are a way in too.
        self.pairing.clear();
        if count > 0 {
            self.hub.publish_revoked();
        }
        count
    }

    pub fn status(&self) -> RemoteStatus {
        let now = now_ms();
        let candidates = self.candidates();
        let lifecycle = self.lifecycle.lock_or_recover();
        let (state, error, bound) = match &*lifecycle {
            Lifecycle::Disabled => (RemoteState::Disabled, None, None),
            Lifecycle::Starting => (RemoteState::Starting, None, None),
            Lifecycle::Active { address, port, .. } => {
                (RemoteState::Active, None, Some((*address, *port)))
            }
            Lifecycle::Error(failure) => (RemoteState::Error, Some(*failure), None),
        };
        drop(lifecycle);
        let base_url = bound.map(|(ip, port)| format!("http://{ip}:{port}"));
        let address_lost =
            bound.is_some_and(|(ip, _)| !candidates.iter().any(|c| c.address == ip.to_string()));
        let pairing = base_url.as_deref().and_then(|base| {
            let issued = self.pairing.current(now)?;
            let url = format!("{base}/#{}", issued.code);
            Some(PairingView {
                code: tokens::display_code(&issued.code),
                qr_svg: qr_svg(&url),
                url,
                expires_at: issued.expires_at,
            })
        });
        RemoteStatus {
            state,
            error,
            url: base_url,
            address: bound.map(|(ip, _)| ip.to_string()),
            port: bound.map(|(_, port)| port),
            address_lost,
            candidates,
            pairing,
            devices: self.sessions.devices(now),
            waiting: u32::try_from(self.gateway.waiting_for_phone()).unwrap_or(u32::MAX),
            server_time: now,
        }
    }
}

/// The QR for a URL, as an SVG that scales to its box and keeps its quiet zone.
fn qr_svg(url: &str) -> String {
    use qrcode::render::svg;
    qrcode::QrCode::new(url.as_bytes()).map_or_else(
        |_| String::new(),
        |code| {
            code.render::<svg::Color<'_>>()
                .min_dimensions(224, 224)
                .quiet_zone(true)
                .dark_color(svg::Color("#000000"))
                .light_color(svg::Color("#ffffff"))
                .build()
        },
    )
}
