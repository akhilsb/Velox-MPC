use lambdaworks_math::traits::ByteConversion;
use protocol::{interpolate_shares};

use crate::Context;

impl Context{
    pub async fn handle_ctrbc_termination(&mut self, _instance_id: usize, sender_rep: usize, content: Vec<u8>){
        // Deserialize message
        let (instance_id,comm_dzk_vals): (usize,(Vec<[u8;32]>,usize)) = bincode::deserialize(content.as_slice()).unwrap();
        log::info!("Received CTRBC termination message from sender {} for instance ID {}",sender_rep,instance_id);

        if !self.sh2t_state_map.contains_key(&instance_id) {
            let sh2t_state = crate::Sh2tState::new();
            self.sh2t_state_map.insert(instance_id, sh2t_state);
        }
        let sh2t_state = self.sh2t_state_map.get_mut(&instance_id).unwrap();

        sh2t_state.commitments.insert(sender_rep, comm_dzk_vals.0);

        // Interpolate shares here for first t parties
        if !self.use_fft && self.myid < 2*self.num_faults{
            // Interpolate your shares in this case
            let secret_key = self.sec_key_map.get(&sender_rep).clone().unwrap().clone();
            let shares = interpolate_shares(secret_key.clone(), comm_dzk_vals.1, false, 1).into_iter().map(|el| el.to_bytes_be()).collect();
            let nonce_share = interpolate_shares(secret_key.clone(),1, true, 1u8)[0].to_bytes_be();
            sh2t_state.shares.insert(sender_rep, (shares,nonce_share));
        }

        log::info!("Deserialization successful for sender {} for instance ID {}",sender_rep,instance_id);
        // If shares already present, then verify shares using this commitment
        if sh2t_state.shares.contains_key(&sender_rep){
            // Verify shares
            self.verify_shares(sender_rep, instance_id).await;
        }
    }
}