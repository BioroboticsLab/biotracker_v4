use super::tracking::TrackingResult;
use super::State;
use anyhow::Result;
use std::collections::HashMap;
use std::net::SocketAddr;
use tokio::io::AsyncWriteExt;
use tokio::net::{TcpListener, TcpStream};

pub struct RobofishCommander {
    listener: TcpListener,
    streams: HashMap<SocketAddr, TcpStream>,
}

impl RobofishCommander {
    pub async fn new(port: u32) -> Result<Self> {
        let addr = format!("127.0.0.1:{}", port);
        let listener = TcpListener::bind(addr).await?;
        Ok(RobofishCommander {
            listener,
            streams: HashMap::new(),
        })
    }

    pub async fn accept(&mut self) -> Result<()> {
        let (socket, addr) = self.listener.accept().await?;
        self.streams.insert(addr, socket);
        Ok(())
    }

    pub async fn send(&mut self, state: &State, result: &TrackingResult) -> Result<()> {
        let frame_number = result.frame_number;
        let fps = state.experiment.target_fps;
        let mut drop_connections = vec![];
        for (addr, stream) in self.streams.iter_mut() {
            let fishcount = result.features.features.len();
            let mut msg = format!("frame:{frame_number};polygon:0;fishcount:{fishcount};");
            let arena = state.experiment.arena.as_ref().expect("Arena not set");

            for feature in &result.features.features {
                if let Some(id) = feature.id {
                    if let Some(pose) = &feature.pose {
                        let orientation_deg = pose.orientation_rad * 180.0 / std::f32::consts::PI;
                        let timestamp_ms = (frame_number as f64 / fps as f64 * 1000.0) as u64;
                        let fish = format!(
                            "{},{},{},{},{},20,20,{},F&",
                            id,
                            pose.x_cm + arena.width_cm as f32 / 2.0,
                            arena.height_cm as f32 / 2.0 - pose.y_cm,
                            pose.orientation_rad,
                            orientation_deg,
                            timestamp_ms
                        );
                        msg += &fish;
                    }
                }
            }
            if fishcount > 0 {
                msg.pop();
            }
            msg += ";end";
            match stream.write_all(msg.as_bytes()).await {
                Ok(_) => {}
                Err(_) => drop_connections.push(addr.clone()),
            }
        }

        for addr in drop_connections {
            log::warn!("Lost connection to Robofish Commander at {}", addr);
            self.streams.remove(&addr);
        }
        Ok(())
    }
}
