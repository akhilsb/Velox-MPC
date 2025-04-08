use std::{collections::HashMap, ops::Add};

use crate::Context;

use bincode::{Result};
use crypto::hash::do_hash;
use lambdaworks_math::{traits::ByteConversion, polynomial::Polynomial};
use protocol::{LargeField, LargeFieldSer, FieldType};
use rayon::prelude::{ ParallelIterator, IntoParallelRefIterator};
use types::{Replica, WrapperMsg};

use crate::{msg::ProtMsg};

impl Context{
    pub async fn init_linear_multiplication_prot(&mut self, mut a_vec_shares: Vec<Vec<LargeField>>, mut b_vec_shares: Vec<Vec<LargeField>>, depth: usize) {
        // Pad shares until they become a multiple of 2t+1
        // Share inputs for later verification
        if depth <= self.max_depth {
            let first_a_shares: Vec<LargeField> = a_vec_shares.clone().into_iter().map(|x| x[0]).collect();
            let first_b_shares: Vec<LargeField> = b_vec_shares.clone().into_iter().map(|x| x[0]).collect();
            log::info!("Adding shares to verification state with a:{} b:{} at depth {}", first_a_shares.len(), first_b_shares.len(), depth);
            self.verf_state.add_mult_inputs(depth, first_a_shares, first_b_shares);
        }
        
        let multiple_of_val = 2*self.num_faults+1;
        let mut padding_length = multiple_of_val - (a_vec_shares.len()%multiple_of_val);
        if (a_vec_shares.len()%multiple_of_val) == 0{
            padding_length =0;
        }
        // Pad the shares until it becomes a multiple of 2t+1
        for _ in 0..padding_length{
            a_vec_shares.push(vec![LargeField::zero()]);
            b_vec_shares.push(vec![LargeField::zero()]);
        }
        if a_vec_shares.len()%multiple_of_val != 0{
            
        }
        let tot_groups = a_vec_shares.len() / (2 * self.num_faults + 1);
        // Use linear multiplication protocol here
        let tot_shares = a_vec_shares.len();
        
        let depth_state;
        if !self.mult_state.depth_share_map.contains_key(&depth){
            depth_state = self.mult_state.get_single_depth_state(depth, true, tot_groups);
        }
        else{
            depth_state = self.mult_state.depth_share_map.get_mut(&depth).unwrap();
        }

        depth_state.padding_shares = padding_length;

        // Get random sharings
        let mut r_sharings = Vec::with_capacity(tot_shares);
        for _ in 0..tot_shares {
            // Check if there are enough random shares
            if self.rand_sharings_state.rand_sharings_mult.len() > 0 {
                
                let rand_sharing = self.rand_sharings_state.rand_sharings_mult.pop_front().unwrap();
                r_sharings.push(rand_sharing.clone());
                depth_state.util_rand_sharings.push(rand_sharing);
            
            } else {
                log::error!("Not enough random shares for linear multiplication protocol");
                return;
            }
        }

        let mut o_sharings = Vec::with_capacity(tot_shares/2);
        for _ in 0..(tot_groups*(self.num_faults+1)) {
            // Check if there are enough random shares for zero multiplication
            if self.rand_sharings_state.rand_2t_sharings_mult.len() > 0 {
                o_sharings.push(self.rand_sharings_state.rand_2t_sharings_mult.pop_front().unwrap());
            } else {
                log::error!("Not enough random shares for zero multiplication protocol");
                return;
            }
        }
            
        // Group inputs
        // let a_vec_shares_grouped = Self::group_elements_by_count(a_vec_shares.clone(), tot_shares / (2 * self.num_faults + 1));
        // let b_vec_shares_grouped = Self::group_elements_by_count(b_vec_shares.clone(), tot_shares / (2 * self.num_faults + 1));
        // let r_shares_grouped = Self::group_elements_by_count(r_sharings.clone(), tot_shares / (2 * self.num_faults + 1));
        // let o_shares_grouped = Self::group_elements_by_count(o_sharings.clone(), tot_shares / (2 * self.num_faults + 1));
        
        let a_vec_shares_grouped;
        let b_vec_shares_grouped;
        let r_shares_grouped;
        let o_shares_grouped;
        if a_vec_shares.len()< 2*self.num_faults+1{
            a_vec_shares_grouped = vec![a_vec_shares];
            b_vec_shares_grouped = vec![b_vec_shares];
            r_shares_grouped = vec![r_sharings];
            o_shares_grouped = vec![o_sharings];
        }
        else{
            a_vec_shares_grouped = a_vec_shares.chunks(2*self.num_faults+1).map(|x|x.to_vec()).collect();
            b_vec_shares_grouped = b_vec_shares.chunks(2*self.num_faults+1).map(|x|x.to_vec()).collect();
            r_shares_grouped = r_sharings.chunks(2*self.num_faults+1).map(|x|x.to_vec()).collect();
            o_shares_grouped = o_sharings.chunks(self.num_faults+1).map(|x|x.to_vec()).collect();
        }
        
        let total_chunks = a_vec_shares_grouped.len();

        // Check that there are the correct number of groups

        let vandermonde_points: Vec<LargeField> = (2..self.num_nodes+2).into_iter().map(|x| LargeField::from(x as u64)).collect();
        let vdm_matrix = Self::vandermonde_matrix(vandermonde_points, self.num_faults); // TODO: can initialize the vdm_matrix somewhere outside to not compute it each time this gets called
                
        let mut shares_party: HashMap<usize, Vec<LargeField>> = HashMap::default();
        for party in 0..self.num_nodes{
            shares_party.insert(party, Vec::with_capacity(tot_shares));
        }
        // Compute all the shares and store them in share_for_party[group][party]
        // Maybe this can be parallelized? 
        for i in 0..total_chunks {
            let o_vec = Self::matrix_vector_multiply(&vdm_matrix, &o_shares_grouped[i]);
            let mut z_vector = Vec::new();
            //zs[i] = Vec::with_capacity(2 * self.num_faults + 1);

            for k in 0..=(2 * self.num_faults) {
                let a: &Vec<LargeField> = &a_vec_shares_grouped[i][k];
                let b: &Vec<LargeField> = &b_vec_shares_grouped[i][k];
                z_vector.push(Self::dot_product(a, b).add(r_shares_grouped[i][k].clone()));
            }
            // Use FFTs here if possible
            let polynomial = Polynomial::new(&z_vector); // Create polynomial from the computed zs
            // Create evaluations at roots of unity?
            // The first level evaluation should still be conducted over normal field elements, the second level evaluation can be conducted over roots of unity
            let evaluations_res 
                = Polynomial::evaluate_fft::<FieldType>(&polynomial, 1, Some(self.num_nodes));
            if evaluations_res.is_err(){
                log::error!("Error evaluating polynomial at roots of unity: {:?}, switching to default evaluation", evaluations_res.err());
                for p in 0..self.num_nodes {
                    let evaluation_point = Self::get_share_evaluation_point(p, self.use_fft, self.roots_of_unity.clone());
                    let share = Self::evaluate_polynomial_from_coefficients_at_position(z_vector.clone(), evaluation_point) + o_vec[p];
                    
                    shares_party.get_mut(&p).unwrap().push(share);
                }
            }
            else{
                let evaluations = evaluations_res.unwrap();
                for (index,share) in (0..self.num_nodes).into_iter().zip(evaluations.into_iter()){
                    shares_party.get_mut(&index).unwrap().push(share + o_vec[index]);
                }
            }
        }

        // Send shares for all groups to all parties
        for (party,shares) in shares_party.into_iter(){
            let ser_shares: Vec<LargeFieldSer> = shares.into_iter().map(|share| {
                share.to_bytes_be()
            }).collect();
            // Encrypt shares before putting them in a message
            let ser_shares_bytes = bincode::serialize(&ser_shares).unwrap();
            let sec_key = self.sec_key_map.get(&party).clone().unwrap();

            // let encrypted_msg = encrypt(sec_key, ser_shares_bytes);
            let prot_msg = ProtMsg::SharesL1(ser_shares_bytes, depth);

            let wrapper_msg = WrapperMsg::new(prot_msg, self.myid, &sec_key);
            let cancel_handler = self.net_send.send(party, wrapper_msg).await;

            self.add_cancel_handler(cancel_handler);
        }
        self.verify_depth_mult_termination(depth).await;
    }

    pub async fn handle_l1_message(&mut self, ser_shares: Vec<u8>, depth: usize, sender: usize) {
        // Try deserializing the message now
        log::info!("Received L1 multiplication shares from party {} for depth {}", sender, depth);
        let shares_option: Result<Vec<LargeFieldSer>> = bincode::deserialize(&ser_shares);
        if shares_option.is_err() {
            log::error!("Error deserializing shares: {:?}", shares_option.err());
            return;
        }

        let shares_ser = shares_option.unwrap();
        
        // Received message as L1 share so multiplication at this depth must be linear
        
        let shares: Vec<LargeField> = shares_ser.into_iter().map(|share| {
            return LargeField::from_bytes_be(&share).unwrap();
        }).collect();

        let depth_state;
        if !self.mult_state.depth_share_map.contains_key(&depth){
            depth_state = self.mult_state.get_single_depth_state(depth, true, shares.len());
        }
        else{
            depth_state = self.mult_state.depth_share_map.get_mut(&depth).unwrap();
        }
        // At L1, the evaluation point is the point at which the polynomials have been evaluated. 
        let evaluation_point = Self::get_share_evaluation_point(sender, self.use_fft.clone(), self.roots_of_unity.clone());
        for (index, share) in shares.into_iter().enumerate(){
            depth_state.l1_shares[index].0.push(evaluation_point);
            depth_state.l1_shares[index].1.push(share);
        }
        
        depth_state.recv_share_count_l1 +=1;
        //depth_state.recv_share_count_l1 = depth_state.recv_share_count_l1.clone().add(1).into();
        let mut ser_shares = None;
        if depth_state.recv_share_count_l1 == self.num_nodes - self.num_faults {
            log::info!("Attempting L1 reconstruction at depth {}", depth);
            // Start reconstruction here
            let secrets: Vec<LargeField> = depth_state.l1_shares.par_iter().map(|(indices,group_shares)|{
                let poly = Polynomial::interpolate(indices, group_shares).unwrap();
                let secret = poly.evaluate(&LargeField::zero()); // Evaluate at zero to get the secret
                return secret;
            }).collect();

            depth_state.l1_shares_reconstructed.extend(secrets.clone());

            let shares_bytes: Vec<LargeFieldSer> = secrets.into_iter().map(|el| el.to_bytes_be()).collect();
            ser_shares = Some(bincode::serialize(&shares_bytes).unwrap());
        }

        if ser_shares.is_some(){
            log::info!("L1 reconstruction successful, sending L2 shares to all parties");
            self.broadcast(ProtMsg::SharesL2(ser_shares.unwrap(), depth)).await;
        }
        self.verify_depth_mult_termination(depth).await;
    }

    pub async fn handle_l2_message(&mut self, group_shares: Vec<u8>, depth: usize, sender: Replica){
        // Multiplication at this depth is of course using two levels of mult
        log::info!("Received L2 multiplication shares from party {} for depth {}", sender, depth);
        let group_shares: Vec<LargeFieldSer> = bincode::deserialize(&group_shares).unwrap();
        
        let depth_state;
        if !self.mult_state.depth_share_map.contains_key(&depth){
            depth_state = self.mult_state.get_single_depth_state(depth, true, group_shares.len());
        }
        else{
            depth_state = self.mult_state.depth_share_map.get_mut(&depth).unwrap();
        }
        
        // At this depth, we are using roots of unity to conduct evaluation
        let evaluation_point = self.roots_of_unity.get(sender).clone().unwrap();

        for (state,group_share) in depth_state.l2_shares.iter_mut().zip(group_shares.into_iter()){
            let group_lf_share = LargeField::from_bytes_be(&group_share).unwrap();
            state.0.push(evaluation_point.clone()); // Store the evaluation point
            state.1.push(group_lf_share); // Store the share itself
        }

        depth_state.recv_share_count_l2 +=1;
        // Interpolate polynomial
        // Idempotence satisfied here
        if depth_state.recv_share_count_l2 == self.num_nodes - self.num_faults{
            log::info!("Attempting L2 reconstruction at depth {}", depth);
            // We have enough shares to reconstruct the polynomial
            let reconstructed_secrets: Vec<LargeField> = depth_state.l2_shares.par_iter().map(|(indices,group_shares)|{
                let poly = Polynomial::interpolate(indices, group_shares).unwrap();
                //let secret = poly.evaluate(&LargeField::zero()); // Evaluate at zero to get the secret
                return poly.coefficients;
            }).flatten().collect();

            depth_state.l2_shares_reconstructed.extend(reconstructed_secrets.clone());
            
            let mut appended_msg = Vec::new();
            for secret in reconstructed_secrets.iter(){
                appended_msg.extend(secret.to_bytes_be());
            }
            let hash = do_hash(&appended_msg);
            log::info!("Completed processing triples at depth {} with linear sharings, broadcasting hash {:?}", depth, hash);
            self.init_hash_broadcast(hash, depth).await;
            self.verify_depth_mult_termination(depth).await;
        }
    }
}