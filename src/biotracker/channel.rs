use anyhow::Result;
use tokio::sync::mpsc;
use tokio::sync::oneshot::{channel, Sender};

#[derive(Debug)]
pub struct ChannelRequest<Req, Res> {
    pub request: Req,
    pub result_tx: Sender<Res>,
}

impl<Req, Res> ChannelRequest<Req, Res>
where
    Req: Send + std::fmt::Debug,
    Res: Send + std::fmt::Debug,
{
    pub async fn send(tx: mpsc::Sender<Self>, request: Req) -> Result<Res> {
        let (result_tx, result_rx) = channel();
        tx.send(Self { request, result_tx })
            .await
            .map_err(|e| anyhow::anyhow!("ChannelRequest failed: {:?}", e))?;
        Ok(result_rx.await?)
    }
}
