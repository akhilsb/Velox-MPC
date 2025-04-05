use protocol::{LargeFieldSer};

use crate::{Context,  Sh2tState};

impl Context{
    pub async fn handle_avid_termination(&mut self, _inst_id: usize, sender: usize, content: Option<Vec<u8>>){
        if content.is_some(){
            let (instance_id,shares) : (usize,(Vec<LargeFieldSer>,LargeFieldSer)) = bincode::deserialize(content.unwrap().as_slice()).unwrap();
            // Deserialize message
            log::info!("Received AVID termination message from sender {} for instance_id {}",sender, instance_id);
            if !self.sh2t_state_map.contains_key(&instance_id) {
                let sh2t_state = Sh2tState::new();
                self.sh2t_state_map.insert(instance_id, sh2t_state);
            }
            let sh2t_state = self.sh2t_state_map.get_mut(&instance_id).unwrap();
            log::info!("Deserialization successful in AVID for sender {}",sender);
            
            sh2t_state.shares.insert(sender, shares);
            self.verify_shares(sender, instance_id).await;
        }
    }
}