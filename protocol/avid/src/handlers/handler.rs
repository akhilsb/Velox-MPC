use async_trait::async_trait;
use futures_util::SinkExt;
use network::Acknowledgement;
use tokio::sync::mpsc::UnboundedSender;

use types::WrapperMsg;

use crate::ProtMsg;

#[derive(Debug, Clone)]
pub struct Handler {
    consensus_tx: UnboundedSender<WrapperMsg<ProtMsg>>,
}

impl Handler {
    pub fn new(consensus_tx: UnboundedSender<WrapperMsg<ProtMsg>>) -> Self {
        Self { consensus_tx }
    }
}

#[async_trait]
impl network::Handler<Acknowledgement, WrapperMsg<ProtMsg>> for Handler {
    async fn dispatch(&self, msg: WrapperMsg<ProtMsg>, writer: &mut network::Writer<Acknowledgement>) {
        // Forward the message
        let status = self.consensus_tx
            .send(msg);
        if status.is_err(){
            log::error!("Failed to send consensus message to the channel because of {:?}", status.err().unwrap());
        }
        // Acknowledge
        let status = writer
            .send(Acknowledgement::Pong)
            .await;
        if status.is_err(){
            log::error!("Failed to send consensus message to the channel because of {:?}", status.err().unwrap());
        }
    }
}
