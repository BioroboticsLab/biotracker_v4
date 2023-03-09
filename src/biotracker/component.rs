use super::{protocol::*, python_process::PythonProcess, ComponentConfig, MatcherService};
use anyhow::Result;
use matcher_server::MatcherServer;
use std::{collections::HashMap, sync::Arc};
use tokio::task::JoinHandle;
use tonic::transport::Channel as ClientChannel;
use tonic::transport::Server;

#[derive(Default)]
pub struct ComponentConnections {
    processes: Vec<PythonProcess>,
    connections: HashMap<ServiceType, ComponentConnection>,
    pending_connections: Vec<JoinHandle<Result<ComponentConnection>>>,
}

impl ComponentConnections {
    pub async fn start_components(&mut self, configs: Vec<ComponentConfig>) -> Result<()> {
        for config in configs {
            self.start_component(config.clone()).await?;
            let service = ServiceType::from_str_name(&config.services[0]).unwrap();
            let task =
                tokio::spawn(async move { ComponentConnection::new(service, &config).await });
            self.pending_connections.push(task);
        }
        Ok(())
    }

    pub async fn stop_components(&mut self) -> Result<()> {
        for mut process in self.processes.drain(..) {
            process.stop().await;
        }
        self.connections.clear();
        self.pending_connections.clear();
        Ok(())
    }

    pub fn has_pending_connections(&self) -> bool {
        !self.pending_connections.is_empty()
    }

    pub async fn update_connections(&mut self) {
        if let Some(task) = self.pending_connections.last_mut() {
            match task.await.unwrap() {
                Ok(connection) => {
                    self.connections.insert(connection.service_type, connection);
                    self.pending_connections.pop();
                }
                Err(err) => {
                    self.pending_connections.pop();
                    log::error!("Failed to connect to component: {}", err);
                }
            }
        }
    }

    pub async fn set_config(&mut self, config: ComponentConfig) -> Result<()> {
        for connection in self.connections.values_mut() {
            if connection.id == config.id {
                connection.set_config(config).await?;
                return Ok(());
            }
        }
        Err(anyhow::anyhow!(
            "No connection found for config {:?}",
            config.id
        ))
    }

    pub fn matcher(&self) -> Option<MatcherClient<ClientChannel>> {
        match self.connections.get(&ServiceType::Matcher) {
            Some(ComponentConnection { client, id, .. }) => match client {
                GrpcClient::Matcher(client) => Some(client.clone()),
                _ => panic!("Component {} is not a matcher", id),
            },
            None => None,
        }
    }

    pub fn feature_detector(&self) -> Option<FeatureDetectorClient<ClientChannel>> {
        match self.connections.get(&ServiceType::FeatureDetector) {
            Some(ComponentConnection { client, id, .. }) => match client {
                GrpcClient::FeatureDetector(client) => Some(client.clone()),
                _ => panic!("Component {} is not a matcher", id),
            },
            None => None,
        }
    }

    pub fn track_recorder(&self) -> Option<TrackRecorderClient<ClientChannel>> {
        match self.connections.get(&ServiceType::TrackRecorder) {
            Some(ComponentConnection { client, id, .. }) => match client {
                GrpcClient::TrackRecorder(client) => Some(client.clone()),
                _ => panic!("Component {} is not a recorder", id),
            },
            None => None,
        }
    }

    async fn start_component(&mut self, config: ComponentConfig) -> Result<()> {
        let address = config.address.to_owned();
        if let Some(python_config) = &config.python_config {
            let process = PythonProcess::start(&config, python_config)?;
            self.processes.push(process);
        } else {
            match config.id.as_str() {
                "HungarianMatcher" => {
                    tokio::spawn(async move {
                        let matcher_service = Arc::new(MatcherService::new());
                        let matcher_server = MatcherServer::from_arc(matcher_service.clone());
                        match Server::builder()
                            .add_service(matcher_server)
                            .serve(address.parse().expect("Invalid address"))
                            .await
                        {
                            Ok(_) => {}
                            Err(e) => {
                                log::warn!("HungarianMatcher failed: {}", e);
                            }
                        };
                    });
                }
                _ => panic!("Unknown component {}", config.id),
            };
        };
        Ok(())
    }
}

pub enum GrpcClient {
    Matcher(MatcherClient<ClientChannel>),
    FeatureDetector(FeatureDetectorClient<ClientChannel>),
    TrackRecorder(TrackRecorderClient<ClientChannel>),
}

pub struct ComponentConnection {
    service_type: ServiceType,
    client: GrpcClient,
    id: String,
}

impl ComponentConnection {
    async fn new(service_type: ServiceType, config: &ComponentConfig) -> Result<Self> {
        let address = format!("http://{}", config.address);
        let channel = ComponentConnection::poll_connect(&address).await?;
        let client = match service_type {
            ServiceType::Matcher => Ok(GrpcClient::Matcher(MatcherClient::new(channel))),
            ServiceType::FeatureDetector => Ok(GrpcClient::FeatureDetector(
                FeatureDetectorClient::new(channel),
            )),
            ServiceType::TrackRecorder => {
                Ok(GrpcClient::TrackRecorder(TrackRecorderClient::new(channel)))
            }
            ServiceType::BiotrackerCore => Err(anyhow::anyhow!("Invalid service name")),
        }?;
        let mut result = Self {
            service_type,
            id: config.id.clone(),
            client,
        };
        result.set_config(config.clone()).await?;
        Ok(result)
    }

    async fn poll_connect(addr: &str) -> Result<tonic::transport::Channel> {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        while std::time::Instant::now() < deadline {
            let addr = addr.to_owned();
            match tonic::transport::Endpoint::new(addr)?.connect().await {
                Ok(conn) => return Ok(conn),
                Err(_) => {}
            }
            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        }
        Err(anyhow::anyhow!("Could not connect to {}", addr))
    }

    pub async fn set_config(&mut self, config: ComponentConfig) -> Result<()> {
        match &mut self.client {
            GrpcClient::Matcher(client) => {
                client.set_config(config).await?;
            }
            GrpcClient::FeatureDetector(client) => {
                client.set_config(config).await?;
            }
            GrpcClient::TrackRecorder(client) => {
                client.set_config(config).await?;
            }
        };
        Ok(())
    }
}
