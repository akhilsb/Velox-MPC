use protocol::{LargeFieldSer};

use crate::{Context, protocol::ACSSABState};

impl Context{
    pub async fn handle_avid_termination(&mut self, instance_id: usize, sender: usize, content: Option<Vec<u8>>){
        log::info!("Received AVID termination message from sender {}",sender);
        if !self.acss_ab_state.contains_key(&instance_id) {
            let acss_state = ACSSABState::new();
            self.acss_ab_state.insert(instance_id, acss_state);
        }
        let acss_state = self.acss_ab_state.get_mut(&instance_id).unwrap();
        // decrypt message
        //let sec_key = self.sec_key_map.get(&sender).unwrap();
        //let shares_ser = decrypt(&sec_key, content.unwrap());
        if content.is_some(){
            let shares : (Vec<LargeFieldSer>,LargeFieldSer,LargeFieldSer) = bincode::deserialize(content.unwrap().as_slice()).unwrap();
            // Deserialize message
            log::info!("Deserialization successful in AVID for sender {}",sender);
            
            acss_state.shares.insert(sender, shares);
            self.verify_shares(sender, instance_id).await;
        }
    }
}