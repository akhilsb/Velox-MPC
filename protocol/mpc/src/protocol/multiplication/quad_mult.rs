use crypto::hash::do_hash;
use lambdaworks_math::{traits::ByteConversion, polynomial::Polynomial};
use protocol::{LargeField, LargeFieldSer};
use rayon::prelude::{IntoParallelIterator, IndexedParallelIterator, ParallelIterator, IntoParallelRefIterator};
use types::Replica;

use crate::{Context, msg::ProtMsg};

impl Context{
    pub async fn quadratic_multiplication_prot(&mut self, a_shares: Vec<Vec<LargeField>>, b_shares: Vec<Vec<LargeField>>, depth: usize){
        log::info!("Starting quadratic multiplication protocol");
        if a_shares.len() != b_shares.len() {
            log::error!("Quadratic multiplication protocol failed: a and b shares length mismatch");
            return;
        }
        let n = a_shares.len();
        let depth_state = self.mult_state.get_single_depth_state(depth, false, n);

        // Log these entries in the verification state for later verification
        if depth <= self.max_depth {
            let first_a_shares = a_shares.clone().into_iter().map(|x| x[0]).collect();
            let first_b_shares = b_shares.clone().into_iter().map(|x| x[0]).collect();
            self.verf_state.add_mult_inputs(depth, first_a_shares, first_b_shares);
        }

        // Poll the r multiplication random shares
        // Pull n shares from r_sharings and n/2 shares from o sharings

        let mut rand_sharings = Vec::new();
        let mut zero_sharings = Vec::new();
        for _ in 0..n{
            if self.rand_sharings_state.rand_sharings_mult.len() > 0 && self.rand_sharings_state.rand_2t_sharings_mult.len()>0{
                rand_sharings.push(self.rand_sharings_state.rand_sharings_mult.pop_front().unwrap());
                zero_sharings.push(self.rand_sharings_state.rand_2t_sharings_mult.pop_front().unwrap());
            } else {
                log::error!("Not enough random shares for multiplication protocol");
                return;
            }
        }

        // Share rand_utils
        depth_state.util_rand_sharings.extend(rand_sharings.clone());
        
        // Perform multiplication
        let mult_shares = 
            (a_shares.into_par_iter()
                .zip(b_shares.into_par_iter()))
            .zip(rand_sharings.into_par_iter()
                .zip(zero_sharings.into_par_iter()))
            .map(|((a,b),(r,o))| (Self::dot_product(&a,&b)+r+o).to_bytes_be())
            .collect::<Vec<LargeFieldSer>>(); // Perform dot product and add random shares

        let ser_shares = bincode::serialize(&mult_shares).unwrap();
        self.broadcast(ProtMsg::QuadShares(ser_shares, depth)).await;
    }

    pub async fn handle_quadratic_mult_shares(&mut self, depth: usize, shares: Vec<u8>, sender: Replica){
        log::info!("Handling quadratic multiplication shares for depth {} from sender {}", depth, sender);
        // Deserialize shares
        let shares_deser = bincode::deserialize::<Vec<LargeFieldSer>>(&shares).unwrap();
        let shares_lf: Vec<LargeField> = shares_deser.into_iter().map(|x| LargeField::from_bytes_be(&x).unwrap()).collect();

        let evaluation_point = Self::get_share_evaluation_point(sender,self.use_fft, self.roots_of_unity.clone());

        // Add shares to the depth state
        let depth_state = self.mult_state.get_single_depth_state(depth, false, shares_lf.len());
        for (share,(indices, shares)) 
                in shares_lf.into_iter().zip(depth_state.l1_shares.iter_mut()){
            indices.push(evaluation_point);
            shares.push(share);
        }

        depth_state.recv_share_count_l1 = depth_state.recv_share_count_l1 + 1; // Increment the count of received shares
        
        if depth_state.recv_share_count_l1 == self.num_nodes - self.num_faults{
            // Reconstruct secrets
            log::info!("Received n-t shares for quadratic protocol reconstruction at depth {}, reconstructing secrets", depth);
            let reconstructed_secrets: Vec<LargeField> 
                = depth_state.l1_shares.par_iter()
                .map(|(indices, evaluations)|{
                    Polynomial::interpolate(
                        indices, // Indices are the evaluation points
                        evaluations // Evaluations are the shares
                    ).unwrap().evaluate(&LargeField::zero())
                }).collect();
            
            depth_state.l1_shares_reconstructed.extend(reconstructed_secrets.clone());
            // Broadcast hash of this reconstructed value. 
            let mut appended_msg = Vec::new();
            for secret in reconstructed_secrets.iter(){
                appended_msg.extend(secret.to_bytes_be());
            }
            let hash = do_hash(&appended_msg);
            log::info!("Completed processing triples at depth {} with quadratic sharings, broadcasting hash {:?}", depth, hash);
            self.broadcast(ProtMsg::HashZMsg(hash,depth,false)).await;
            self.verify_depth_mult_termination(depth).await;
        }
    }
}