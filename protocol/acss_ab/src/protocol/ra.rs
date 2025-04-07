use crypto::aes_hash::MerkleTree;
use lambdaworks_math::{traits::ByteConversion, polynomial::Polynomial};
use protocol::LargeField;

use crate::{Context, protocol::ACSSABState};

impl Context{
    pub async fn handle_ra_termination(&mut self, instance_id: usize, sender: usize, value: usize){
        log::info!("Received RA termination message from sender {} with value {}",sender, value);
        if !self.acss_ab_state.contains_key(&instance_id) {
            let acss_state = ACSSABState::new();
            self.acss_ab_state.insert(instance_id, acss_state);
        }
        let acss_state = self.acss_ab_state.get_mut(&instance_id).unwrap();
        if value == 1{
            acss_state.ra_outputs.insert(sender);
        }
        // Send shares back to parent process
        self.check_termination(sender, instance_id).await;
    }

    pub async fn check_termination(&mut self, sender:usize, instance_id: usize){
        let acss_state = self.acss_ab_state.get_mut(&instance_id).unwrap();

        if acss_state.acss_status.contains(&sender){
            return;
        }
        if acss_state.shares.contains_key(&sender) 
        && acss_state.ra_outputs.contains(&sender) 
        && acss_state.verification_status.contains_key(&sender){
            if acss_state.verification_status.get(&sender).unwrap().clone(){
                // Send shares back to parent process
                log::info!("Sending shares back to syncer for sender {} for instance id {}",sender, instance_id);
                if self.avss_inst_id == instance_id{
                    acss_state.acss_status.insert(sender);
                    let shares = acss_state.shares.get(&sender).unwrap().clone();
                    let (comm,b_comm, dzk_poly) = acss_state.commitments.get(&sender).unwrap().clone();

                    // Compute root commitment
                    let share_root = MerkleTree::new(comm, &self.hash_context).root();
                    let blinding_root = MerkleTree::new(b_comm, &self.hash_context).root();
                    let root_comm = self.hash_context.hash_two(share_root, blinding_root);
                    let root_comm_fe = LargeField::from_bytes_be(&root_comm).unwrap();
                    acss_state.commitment_root_fe.insert(sender, root_comm_fe);

                    // Compute DZK polynomial
                    let dzk_poly_coeffs: Vec<LargeField> = dzk_poly.into_iter().map(|el| LargeField::from_bytes_be(el.as_slice()).unwrap()).collect();
                    let dzk_poly = Polynomial::new(dzk_poly_coeffs.as_slice());
                    acss_state.dzk_poly.insert(sender, dzk_poly);

                    let _status = self.out_avss.send((true,Some((sender,shares)),None)).await;
                }
                else{
                    let shares = acss_state.shares.get(&sender).unwrap().clone();
                    let _status = self.out_acss.send((instance_id,sender,Some(shares.0))).await;
                    acss_state.acss_status.insert(sender);
                }
                //self.terminate("Hello".to_string()).await;
            }
            else{
                let _status = self.out_acss.send((instance_id, sender,None)).await;
                acss_state.acss_status.insert(sender);
            }
        }
    }
    // Invoke this function once you terminate the protocol
    // pub async fn terminate(&mut self, data: String) {
    //     let rbc_sync_msg = RBCSyncMsg{
    //         id: 1,
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