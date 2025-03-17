use protocol::{interpolate_shares};

use crate::Context;

impl Context{
    pub async fn handle_ctrbc_termination(&mut self, inst_id: usize, sender_rep: usize, content: Vec<u8>){
        log::info!("Received CTRBC termination message from sender {} for instance ID {}",sender_rep,inst_id);

        // Deserialize message
        let comm_dzk_vals: (Vec<[u8;32]>,Vec<[u8;32]>,Vec<[u8;32]>,usize) = bincode::deserialize(content.as_slice()).unwrap();
        self.acss_ab_state.commitments.insert(sender_rep, (comm_dzk_vals.0,comm_dzk_vals.1,comm_dzk_vals.2));

        // Interpolate shares here for first t parties
        if !self.use_fft && self.myid < self.num_faults{
            // Interpolate your shares in this case
            let secret_key = self.sec_key_map.get(&sender_rep).clone().unwrap().clone();
            let shares = interpolate_shares(secret_key.clone(), comm_dzk_vals.3, false, 1).into_iter().map(|el| el.to_bytes_be()).collect();
            let nonce_share = interpolate_shares(secret_key.clone(),1, true, 1u8)[0].to_bytes_be();
            let blinding_nonce_share = interpolate_shares(secret_key, 1, true, 3u8)[0].to_bytes_be();
            self.acss_ab_state.shares.insert(sender_rep, (shares,nonce_share,blinding_nonce_share));
        }

        log::info!("Deserialization successful for sender {} for instance ID {}",sender_rep,inst_id);
        // If shares already present, then verify shares using this commitment
        if self.acss_ab_state.shares.contains_key(&sender_rep){
            // Verify shares
            self.verify_shares(sender_rep).await;
        }
    }
}