//! The computer's network interfaces, as the operating system reports them.

use crate::application::remote::network::{InterfaceAddress, NetworkInterfaces};

pub struct SystemInterfaces;

impl NetworkInterfaces for SystemInterfaces {
    fn addresses(&self) -> Vec<InterfaceAddress> {
        // A failure to list is "no addresses": the dialog then says there is no network.
        if_addrs::get_if_addrs()
            .unwrap_or_default()
            .into_iter()
            .map(|interface| InterfaceAddress {
                interface: interface.name.clone(),
                ip: interface.ip(),
            })
            .collect()
    }
}
