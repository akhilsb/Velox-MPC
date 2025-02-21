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
                let shares = self.acss_ab_state.shares.get(&sender).unwrap().clone();
                let _status = self.out_acss.send((sender,Some(shares.0))).await;
                self.acss_ab_state.acss_status.insert(sender);
            }
            else{
                let _status = self.out_acss.send((sender,None)).await;
                self.acss_ab_state.acss_status.insert(sender);
            }
        }
    }
}