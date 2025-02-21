use crate::Context;

impl Context{
    pub async fn handle_ctrbc_termination(&mut self, inst_id: usize, sender_rep: usize, content: Vec<u8>){
        log::info!("Received CTRBC termination message from sender {} for instance ID {}",sender_rep,inst_id);

        // Deserialize message
        let comm_dzk_vals: (Vec<[u8;32]>,Vec<[u8;32]>,Vec<[u8;32]>) = bincode::deserialize(content.as_slice()).unwrap();
        self.acss_ab_state.commitments.insert(sender_rep, comm_dzk_vals);


        log::info!("Deserialization successful for sender {} for instance ID {}",sender_rep,inst_id);
        // If shares already present, then verify shares using this commitment
        
        if self.acss_ab_state.shares.contains_key(&sender_rep){
            // Verify shares
            self.verify_shares(sender_rep).await;
        }
    }
}