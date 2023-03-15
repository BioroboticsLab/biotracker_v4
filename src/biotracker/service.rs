use super::{
    bio_tracker_server::BioTracker,
    protocol::{Command, Empty, Experiment, Image},
    BioTrackerCommand, ChannelRequest,
};
use anyhow::Result;
use tokio::sync::mpsc::Sender;
use tonic::{Request, Response, Status};

pub struct Service {
    pub command_tx: Sender<ChannelRequest<Command, Result<Empty>>>,
    pub state_tx: Sender<ChannelRequest<(), Experiment>>,
    pub image_tx: Sender<ChannelRequest<Image, Result<Empty>>>,
}

#[tonic::async_trait]
impl BioTracker for Service {
    async fn get_state(&self, _: Request<Empty>) -> Result<Response<Experiment>, Status> {
        Ok(Response::new(
            ChannelRequest::send(self.state_tx.clone(), ())
                .await
                .map_err(|e| Status::internal(format!("{}", e)))?,
        ))
    }

    async fn command(
        &self,
        request: Request<BioTrackerCommand>,
    ) -> Result<Response<Empty>, Status> {
        Ok(Response::new(
            ChannelRequest::send(
                self.command_tx.clone(),
                request.into_inner().command.unwrap(),
            )
            .await
            .map_err(|e| Status::internal(format!("{}", e)))?
            .map_err(|e| Status::invalid_argument(format!("{}", e)))?,
        ))
    }

    async fn add_image(&self, request: Request<Image>) -> Result<Response<Empty>, Status> {
        Ok(Response::new(
            ChannelRequest::send(self.image_tx.clone(), request.into_inner())
                .await
                .map_err(|e| Status::internal(format!("{}", e)))?
                .map_err(|e| Status::invalid_argument(format!("{}", e)))?,
        ))
    }

    async fn heartbeat(&self, _: Request<Empty>) -> Result<Response<Empty>, Status> {
        Ok(Response::new(Empty {}))
    }
}
