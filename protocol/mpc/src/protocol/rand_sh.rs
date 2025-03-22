use protocol::{rand_field_element, LargeField};
use crate::{context::Context, msg::ProtMsg};

impl Context{
    pub async fn init_rand_sh(&mut self, batch_size: usize, num_batches: usize){
        // Start ACSS with abort and 2t-sharing simultaneously for each batch
        for batch in 0..num_batches{
            // Create random values
            let mut rand_values = Vec::new();
            let mut zeros = Vec::new();
            for _ in 0..batch_size{
                rand_values.push(rand_field_element().to_bytes_be());
                zeros.push(LargeField::from(0 as u64).to_bytes_be());
            }

            log::info!("Initiating secret sharing in preprocessing phase for batch {}", batch);
            let _status = self.acss_ab_send.send((batch,rand_values)).await;
            let _status = self.sh2t_send.send((batch,zeros)).await;

            self.broadcast(ProtMsg::ReconstructCoin()).await;
        }
    }
}