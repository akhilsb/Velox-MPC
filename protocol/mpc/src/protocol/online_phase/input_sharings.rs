use std::collections::HashMap;

use lambdaworks_math::traits::ByteConversion;
use protocol::{LargeFieldSer, LargeField};
use types::Replica;

use crate::Context;

impl Context{
    pub async fn handle_input_acss_termination(&mut self, instance_id: usize, sender: Replica, shares: Option<Vec<LargeFieldSer>>){
        log::info!("Received input ACSS termination message from sender {} for instance ID {}", sender, instance_id);
        if shares.is_none(){
            log::error!("Abort ACSS protocol of dealer {} and terminate MPC", sender);
            return;
        }

        let shares = shares.unwrap();
        let input_sharing_inst = instance_id - self.input_acss_id_offset;

        if !self.mix_circuit_state.input_acss_shares.contains_key(&sender){
            let input_sharing_state = HashMap::default();
            self.mix_circuit_state.input_acss_shares.insert(sender, input_sharing_state);
        }
        
        let input_sharing_state = self.mix_circuit_state.input_acss_shares.get_mut(&sender).unwrap();
        let shares_deser: Vec<LargeField> = shares.into_iter()
            .map(|el| LargeField::from_bytes_be(&el).unwrap())
            .collect();

        input_sharing_state.insert(input_sharing_inst, shares_deser);
        self.verify_sender_termination(sender).await;
    }
}