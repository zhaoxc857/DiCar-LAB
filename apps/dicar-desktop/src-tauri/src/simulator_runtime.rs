use std::net::{IpAddr, Ipv4Addr, SocketAddr};

use dctp_sim::SimulatorServer;
use dicar_app_core::CoreConfig;

use crate::{AppState, BridgeErrorDto};

pub struct BundledSimulator {
    server: SimulatorServer,
}

impl BundledSimulator {
    pub fn local_addr(&self) -> SocketAddr {
        self.server.local_addr()
    }
}

pub fn spawn_bundled_runtime() -> Result<(BundledSimulator, AppState), BridgeErrorDto> {
    let server = SimulatorServer::spawn(SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 0))
        .map_err(|error| {
            BridgeErrorDto::new(
                "simulatorUnavailable",
                format!("无法启动内置模拟器：{error}"),
            )
        })?;
    let state = AppState::spawn(CoreConfig::simulator(server.local_addr()))?;
    Ok((BundledSimulator { server }, state))
}
