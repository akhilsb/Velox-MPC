use crypto::{hash::do_hash};
use lambdaworks_math::{traits::ByteConversion,};
use protocol::{LargeFieldSer, LargeField, AvssShare};
use types::Replica;

use crate::Context;

impl Context{
    pub async fn init_avss(&mut self, secrets: Vec<LargeFieldSer>){
        // Use the avss instance id for this
        log::info!("Initializing AVSS with instance id {}", self.acss_id+1);

        let secrets_deser: Vec<LargeField> = secrets.into_iter().map(|x| LargeField::from_bytes_be(&x).unwrap()).collect::<Vec<LargeField>>();
        self.init_acss_ab(secrets_deser, self.avss_inst_id).await;
    }

    pub async fn share_validity_oracle(&mut self, origin: Replica, share_sender: Replica, share: AvssShare){
        // Use the avss instance id for this
        let orig_share = share.clone();
        log::info!("Request received to validate shares from sender {} for AVSS from origin {}", share_sender, origin);
        if !self.acss_ab_state.contains_key(&self.avss_inst_id){
            log::error!("AVSS instance not found");
            return;
        }
        let avss_state = self.acss_ab_state.get(&self.avss_inst_id).unwrap();
        if !avss_state.commitments.contains_key(&origin){
            return;
        }
        
        let (commitments, blinding_commitments, _dzk_poly) = avss_state.commitments.get(&origin).unwrap().clone();

        let shares = share.0;
        let nonce = share.1;
        let blinding_nonce = share.2;
        // Compute commitment and DZK proof
        let mut appended_vec = Vec::new();
        for share in shares.iter(){
            appended_vec.extend_from_slice(share);
        }
        appended_vec.extend_from_slice(&nonce);
        // Compute Hash

        let commitment = do_hash(&appended_vec);
        if commitment != commitments[share_sender]{
            log::error!("Commitment mismatch for share from sender {} for AVSS from origin {}", share_sender, origin);
            return;
        }

        let deser_shares = shares.iter().map(|x| LargeField::from_bytes_be(x).unwrap()).collect::<Vec<LargeField>>();
        
        let dzk_poly  = avss_state.dzk_poly.get(&origin).unwrap();
        let root_comm_fe = avss_state.commitment_root_fe.get(&origin).unwrap();

        let status = self.evaluate_dzk_poly(
            root_comm_fe.clone(), 
            share_sender,
            dzk_poly,
            &deser_shares, 
            blinding_commitments[share_sender].clone(), 
            blinding_nonce.clone()
        );

        if !status{
            log::error!("DZK proof mismatch for share from sender {} for AVSS from origin {}", share_sender, origin);
            return;
        }
        else{
            // send share back through the channel
            log::info!("Successfully validated AVSS share, sending output back to channel");
            let _status = self.out_avss.send((false, None, Some((origin, share_sender, orig_share)))).await;
        }
    }
}