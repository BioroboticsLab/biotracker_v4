use crate::biotracker::protocol::*;
use anyhow::Result;
use std::sync::Arc;

pub struct BioTrackerController {
    client: bio_tracker_client::BioTrackerClient<tonic::transport::Channel>,
    rt: Arc<tokio::runtime::Runtime>,
}

impl BioTrackerController {
    pub fn new(addr: String, rt: Arc<tokio::runtime::Runtime>) -> Self {
        let client = rt
            .block_on(async {
                for _ in 0..10 {
                    match bio_tracker_client::BioTrackerClient::connect(addr.clone()).await {
                        Ok(client) => return Ok(client),
                        Err(_) => {}
                    }
                    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                }
                return Err(anyhow::anyhow!("Failed to connect to BioTracker Core"));
            })
            .unwrap();
        Self { client, rt }
    }

    pub fn get_state(&mut self) -> Result<Experiment> {
        let BioTrackerController { client, rt } = self;
        let response = rt.block_on(async move {
            let request = tonic::Request::new(Empty {});
            client.get_state(request).await
        });
        Ok(response?.into_inner())
    }

    pub fn command(&mut self, command: Command) -> Result<Empty> {
        let BioTrackerController { client, rt } = self;
        let response = rt.block_on(async move {
            let biotracker_command = BioTrackerCommand {
                command: Some(command),
            };
            let request = tonic::Request::new(biotracker_command);
            client.command(request).await
        });
        Ok(response?.into_inner())
    }

    pub fn add_image(&mut self, image: Image) -> Result<Empty> {
        let BioTrackerController { client, rt } = self;
        let response = rt.block_on(async move {
            let request = tonic::Request::new(image);
            client.add_image(request).await
        });
        Ok(response?.into_inner())
    }
}
