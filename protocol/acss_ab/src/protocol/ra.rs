use types::{RBCSyncMsg, SyncMsg, SyncState};

use crate::Context;

impl Context{
    pub async fn handle_ra_termination(&mut self, _inst_id: usize, sender: usize, value: usize){
        log::info!("Received RA termination message from sender {} with value {}",sender, value);
        if value == 1{
            self.acss_ab_state.ra_outputs.insert(sender);
        }
        // Send shares back to parent process
        self.check_termination(sender).await;
    }

    pub async fn check_termination(&mut self, sender:usize){
        if self.acss_ab_state.acss_status.contains(&sender){
            return;
        }
        if self.acss_ab_state.shares.contains_key(&sender) 
        && self.acss_ab_state.ra_outputs.contains(&sender) 
        && self.acss_ab_state.verification_status.contains_key(&sender){
            if self.acss_ab_state.verification_status.get(&sender).unwrap().clone(){
                // Send shares back to parent process
                log::info!("Sending shares back to syncer for sender {}",sender);
                let shares = self.acss_ab_state.shares.get(&sender).unwrap().clone();
                let _status = self.out_acss.send((sender,Some(shares.0))).await;
                self.acss_ab_state.acss_status.insert(sender);
                self.terminate("Hello".to_string()).await;
            }
            else{
                let _status = self.out_acss.send((sender,None)).await;
                self.acss_ab_state.acss_status.insert(sender);
            }
        }
    }

    // Invoke this function once you terminate the protocol
    pub async fn terminate(&mut self, data: String) {
        let rbc_sync_msg = RBCSyncMsg{
            id: 1,
            msg: data,
        };

        let ser_msg = bincode::serialize(&rbc_sync_msg).unwrap();
        let cancel_handler = self
            .sync_send
            .send(
                0,
                SyncMsg {
                    sender: self.myid,
                    state: SyncState::COMPLETED,
                    value: ser_msg,
                },
            )
            .await;
        self.add_cancel_handler(cancel_handler);
    }
}