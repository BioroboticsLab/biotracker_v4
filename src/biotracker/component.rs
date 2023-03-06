use super::protocol::*;
use anyhow::Result;
use std::collections::HashMap;
use tonic::transport::Channel as ClientChannel;

#[derive(Default)]
pub struct ComponentConnections {
    connections: HashMap<ServiceType, ComponentConnection>,
}

impl ComponentConnections {
    pub async fn new(configs: Vec<ComponentConfig>) -> Result<Self> {
        let mut connections = HashMap::new();
        for config in configs {
            let service = ServiceType::from_str_name(&config.services[0]).unwrap();
            let connection = ComponentConnection::new(service, &config).await?;
            connections.insert(service, connection);
        }
        Ok(Self { connections })
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

    pub fn matcher(&self) -> MatcherClient<ClientChannel> {
        match &self.get_component(ServiceType::Matcher).client {
            GrpcClient::Matcher(client) => client.clone(),
            _ => panic!("Invalid client type"),
        }
    }

    pub fn feature_detector(&self) -> FeatureDetectorClient<ClientChannel> {
        match &self.get_component(ServiceType::FeatureDetector).client {
            GrpcClient::FeatureDetector(client) => client.clone(),
            _ => panic!("Invalid client type"),
        }
    }

    pub fn track_recorder(&mut self) -> TrackRecorderClient<ClientChannel> {
        match &self.get_component(ServiceType::TrackRecorder).client {
            GrpcClient::TrackRecorder(client) => client.clone(),
            _ => panic!("Invalid client type"),
        }
    }

    fn get_component(&self, service_type: ServiceType) -> &ComponentConnection {
        match self.connections.get(&service_type) {
            Some(connection) => connection,
            None => panic!("No connection found for service type {:?}", service_type),
        }
    }
}

pub enum GrpcClient {
    Matcher(MatcherClient<ClientChannel>),
    FeatureDetector(FeatureDetectorClient<ClientChannel>),
    TrackRecorder(TrackRecorderClient<ClientChannel>),
}

pub struct ComponentConnection {
    pub service_type: ServiceType,
    pub client: GrpcClient,
    pub id: String,
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
