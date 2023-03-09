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

    pub fn command(&mut self, command: Command) {
        match self.command_request(command.clone()) {
            Ok(_) => {}
            Err(e) => {
                log::error!(target: "", "{}: {}", error_message(&command), e.message());
            }
        }
    }

    fn command_request(
        &mut self,
        command: Command,
    ) -> Result<tonic::Response<Empty>, tonic::Status> {
        let BioTrackerController { client, rt } = self;
        let response = rt.block_on(async move {
            let biotracker_command = BioTrackerCommand {
                command: Some(command),
            };
            let request = tonic::Request::new(biotracker_command);
            client.command(request).await
        });
        response
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

fn error_message(command: &Command) -> String {
    match command {
        Command::PlaybackState(state) => format!("Failed to set playback state to {}", state),
        Command::RecordingState(state) => format!("Failed to set recording state to {}", state),
        Command::RealtimeMode(state) => format!("Failed to set realtime mode to {}", state),
        Command::UndistortMode(mode) => format!("Failed to set undistortion mode to {}", mode),
        Command::TargetFps(fps) => format!("Failed to set target fps to {}", fps),
        Command::Seek(frame) => format!("Failed to seek to frame {}", frame),
        Command::OpenVideo(path) => format!("Failed to open video {}", path),
        Command::OpenTrack(path) => format!("Failed to open track {}", path),
        Command::InitializeRecording(config) => {
            format!("Failed to initialize recording with config {:?}", config)
        }
        Command::AddEntity(_) => format!("Failed to add entity"),
        Command::RemoveEntity(_) => format!("Failed to remove"),
        Command::SwitchEntities(idswitch) => {
            format!(
                "Failed to switch entities {} and {}",
                idswitch.id1, idswitch.id2
            )
        }
        Command::UpdateArena(arena) => format!("Failed to update arena {:?}", arena),
        Command::UpdateComponent(component) => {
            format!("Failed to set component config {:?}", component)
        }
        Command::SaveConfig(_) => format!("Failed to save config"),
        Command::Shutdown(_) => format!("Failed to shutdown"),
    }
}
