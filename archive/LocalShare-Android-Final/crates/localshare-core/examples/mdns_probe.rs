use mdns_sd::{ServiceDaemon, ServiceInfo};
use std::{thread, time::Duration};

fn main() {
    let daemon = ServiceDaemon::new().expect("create mDNS daemon");
    let properties = [("id", "generated-probe"), ("name", "Generated Probe")];
    let service = ServiceInfo::new(
        "_localshare._tcp.local.",
        "Generated-Probe",
        "Generated-Probe.local.",
        "",
        33399,
        &properties[..],
    )
    .expect("create service")
    .enable_addr_auto();
    println!("registering {}", service.get_fullname());
    daemon.register(service).expect("register service");
    thread::sleep(Duration::from_secs(20));
    let _ = daemon.shutdown();
}
