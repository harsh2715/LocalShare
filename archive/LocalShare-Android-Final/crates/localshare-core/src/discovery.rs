use crate::{CoreEvent, LocalShareCore, Peer};
use anyhow::{anyhow, Result};
use mdns_sd::{DaemonEvent, ServiceDaemon, ServiceEvent, ServiceInfo};
use std::net::IpAddr;
use std::sync::Arc;
use tokio::sync::oneshot;

pub const SERVICE_TYPE: &str = "_localshare._tcp.local.";

pub struct DiscoveryHandle {
    stop: Option<oneshot::Sender<()>>,
}

impl Drop for DiscoveryHandle {
    fn drop(&mut self) {
        if let Some(stop) = self.stop.take() {
            let _ = stop.send(());
        }
    }
}

pub async fn start(core: Arc<LocalShareCore>) -> Result<DiscoveryHandle> {
    let mdns = ServiceDaemon::new().map_err(|e| anyhow!("mDNS start failed: {e}"))?;
    let status = core.status();
    let instance = format!("{}-{}", core.identity().name, &core.identity().id[..8]);
    let version = crate::protocol::VERSION.to_string();
    let properties = [
        ("id", core.identity().id.as_str()),
        ("name", core.identity().name.as_str()),
        ("version", version.as_str()),
        ("fingerprint", core.identity().fingerprint.as_str()),
    ];
    let configured_ip = core.config().bind_host.parse::<IpAddr>().ok();
    let advertise_ip = configured_ip
        .filter(|ip| !ip.is_unspecified())
        .map(|ip| ip.to_string())
        .unwrap_or_default();
    let monitor = mdns
        .monitor()
        .map_err(|e| anyhow!("mDNS monitor failed: {e}"))?;
    let monitor_core = core.clone();
    tokio::spawn(async move {
        while let Ok(event) = monitor.recv_async().await {
            if let DaemonEvent::Error(error) = event {
                eprintln!("LocalShare mDNS daemon error: {error}");
                monitor_core.emit(CoreEvent::Error(format!("mDNS daemon error: {error}")));
            }
        }
    });
    let service = ServiceInfo::new(
        SERVICE_TYPE,
        &instance,
        &format!("{}{}", instance.replace(' ', "-"), SERVICE_TYPE),
        if advertise_ip.is_empty() {
            "".to_owned()
        } else {
            advertise_ip
        },
        status.listener_port,
        &properties[..],
    )
    .map_err(|e| anyhow!("mDNS service creation failed: {e}"))?;
    let service = if configured_ip.is_none() || configured_ip.is_some_and(|ip| ip.is_unspecified())
    {
        service.enable_addr_auto()
    } else {
        service
    };
    eprintln!(
        "LocalShare mDNS registering {} on port {}",
        service.get_fullname(),
        status.listener_port
    );
    mdns.register(service)
        .map_err(|e| anyhow!("mDNS register failed: {e}"))?;
    let receiver = mdns
        .browse(SERVICE_TYPE)
        .map_err(|e| anyhow!("mDNS browse failed: {e}"))?;
    let (stop, mut stop_rx) = oneshot::channel();
    tokio::spawn(async move {
        loop {
            tokio::select! {
                _ = &mut stop_rx => break,
                event = receiver.recv_async() => match event {
                    Ok(ServiceEvent::ServiceResolved(info)) => {
                        let Some(id) = info.get_property_val_str("id") else { continue };
                        if id == core.identity().id { continue; }
                        let Some(name) = info.get_property_val_str("name") else { continue };
                        let Some(fingerprint) = info.get_property_val_str("fingerprint") else { continue; };
                        let allow_loopback = core.config().bind_host.parse::<IpAddr>().map(|ip| ip.is_loopback()).unwrap_or(false);
                        let Some(address) = info.get_addresses().iter().find(|address| allow_loopback || !address.is_loopback()) else { continue; };
                        let peer = Peer { id: id.to_owned(), name: name.to_owned(), host: address.to_string(), port: info.get_port(), fingerprint: fingerprint.to_owned(), protocol_version: info.get_property_val_str("version").and_then(|v| v.parse().ok()).unwrap_or(crate::protocol::VERSION), capabilities: vec!["chat".into(), "files".into()], state: "DISCOVERED".into() };
                        let _ = core.store().save_peer(&peer);
                        core.emit(CoreEvent::Peer(peer));
                    }
                    Ok(ServiceEvent::ServiceRemoved(_, _)) => {}
                    Ok(_) => {}
                    Err(_) => break,
                }
            }
        }
        let _ = mdns.shutdown();
    });
    Ok(DiscoveryHandle { stop: Some(stop) })
}
