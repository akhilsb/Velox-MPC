//use types::{RBCSyncMsg, SyncMsg, SyncState};

use crate::Context;

impl Context{
    pub async fn handle_ra_termination(&mut self, instance_id: usize, sender: usize, value: usize){
        log::info!("Received RA termination message from sender {} with value {}",sender, value);
        if !self.sh2t_state_map.contains_key(&instance_id){
            let sh2t_state = crate::Sh2tState::new();
            self.sh2t_state_map.insert(instance_id, sh2t_state);
        }
        let sh2t_state = self.sh2t_state_map.get_mut(&instance_id).unwrap();
        if value == 1{
            sh2t_state.ra_outputs.insert(sender);
        }
        // Send shares back to parent process
        self.check_termination(sender, instance_id).await;
    }

    pub async fn check_termination(&mut self, sender:usize, instance_id: usize){
        if !self.sh2t_state_map.contains_key(&instance_id){
            let sh2t_state = crate::Sh2tState::new();
            self.sh2t_state_map.insert(instance_id, sh2t_state);
        }
        let sh2t_state = self.sh2t_state_map.get_mut(&instance_id).unwrap();

        if sh2t_state.status.contains(&sender){
            return;
        }
        if sh2t_state.shares.contains_key(&sender) 
        && sh2t_state.ra_outputs.contains(&sender) 
        && sh2t_state.verification_status.contains_key(&sender){
            if sh2t_state.verification_status.get(&sender).unwrap().clone(){
                // Send shares back to parent process
                log::info!("Sending shares back to syncer for sender {} and instance id: {}",sender, instance_id);
                let shares = sh2t_state.shares.get(&sender).unwrap().clone();
                let _status = self.out_acss.send((instance_id, sender,Some(shares.0))).await;
                sh2t_state.status.insert(sender);
                // self.terminate("Hello".to_string(), instance_id, sender).await;
            }
            else{
                let _status = self.out_acss.send((instance_id, sender,None)).await;
                sh2t_state.status.insert(sender);
            }
        }
    }

    // Invoke this function once you terminate the protocol
    // pub async fn terminate(&mut self, data: String, instance_id: usize, sender: usize) {
    //     let rbc_sync_msg = RBCSyncMsg{
    //         id: instance_id+sender,
    //         msg: data,
    //     };

    //     let ser_msg = bincode::serialize(&rbc_sync_msg).unwrap();
    //     let cancel_handler = self
    //         .sync_send
    //         .send(
    //             0,
    //             SyncMsg {
    //                 sender: self.myid,
    //                 state: SyncState::COMPLETED,
    //                 value: ser_msg,
    //             },
    //         )
    //         .await;
    //     self.add_cancel_handler(cancel_handler);
    // }
}