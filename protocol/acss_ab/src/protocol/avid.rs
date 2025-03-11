use crypto::decrypt;
use protocol::{LargeFieldSer};

use crate::Context;

impl Context{
    pub async fn handle_avid_termination(&mut self, sender: usize, content: Option<Vec<u8>>){
        log::info!("Received AVID termination message from sender {}",sender);

        // decrypt message
        let sec_key = self.sec_key_map.get(&sender).unwrap();
        let shares_ser = decrypt(&sec_key, content.unwrap());

        let shares : (Vec<LargeFieldSer>,LargeFieldSer,LargeFieldSer) = bincode::deserialize(shares_ser.as_slice()).unwrap();
        // Deserialize message
        log::info!("Deserialization successful in AVID for sender {}",sender);
        self.acss_ab_state.shares.insert(sender, shares);
        self.verify_shares(sender).await;
    }
}