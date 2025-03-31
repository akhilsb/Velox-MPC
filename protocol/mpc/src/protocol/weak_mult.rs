use std::{collections::HashMap, ops::Add};

use bincode::Result;
use crypto::{encrypt, decrypt};
use lambdaworks_math::{polynomial::Polynomial, traits::ByteConversion};
use protocol::{LargeField, LargeFieldSer, FieldType};
use rayon::prelude::{IntoParallelRefIterator, IndexedParallelIterator, ParallelIterator, IntoParallelIterator};
use types::{WrapperMsg, Replica};

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
            // Now, subtract random sharings from the reconstructed secrets
            let _next_depth_sharings: Vec<LargeField> = 
                reconstructed_secrets.into_iter()
                    .zip(depth_state.util_rand_sharings.iter())
                    .map(|(recon,sharing)| 
                        recon-sharing.clone())
                        .collect();
            
            // Do something with these sharings here
        }
    }

    pub async fn linear_multiplication_prot(&mut self, a_vec_shares: Vec<Vec<Option<LargeField>>>, b_vec_shares: Vec<Vec<Option<LargeField>>>, depth: usize, lin_or_quadratic: bool) {
        let tot_shares = a_vec_shares.len();
        let tot_groups = tot_shares / (2 * self.num_faults + 1);
        
        let depth_state = self.mult_state.get_single_depth_state(depth, lin_or_quadratic, tot_groups);

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
        for _ in 0..tot_shares/2 {
            // Check if there are enough random shares for zero multiplication
            if self.rand_sharings_state.rand_2t_sharings_mult.len() > 0 {
                o_sharings.push(self.rand_sharings_state.rand_2t_sharings_mult.pop_front().unwrap());
            } else {
                log::error!("Not enough random shares for zero multiplication protocol");
                return;
            }
        }

        // Initialize
        let _cs = vec![vec![Some(LargeField::zero()); 2*self.num_faults + 1]; tot_shares / (2 * self.num_faults + 1)];

        // Group inputs
        let a_vec_shares_grouped = Self::group_elements_by_count(a_vec_shares.clone(), tot_shares / (2 * self.num_faults + 1));
        let b_vec_shares_grouped = Self::group_elements_by_count(b_vec_shares.clone(), tot_shares / (2 * self.num_faults + 1));
        let r_shares_grouped = Self::group_elements_by_count(r_sharings.clone(), tot_shares / (2 * self.num_faults + 1));
        let o_shares_grouped = Self::group_elements_by_count(o_sharings.clone(), tot_shares / (2 * self.num_faults + 1));
        // Check that there are the correct number of groups
        assert_eq!(a_vec_shares_grouped.len(), tot_shares / (2 * self.num_faults + 1));
        assert_eq!(b_vec_shares_grouped.len(), tot_shares / (2 * self.num_faults + 1));
        assert_eq!(r_shares_grouped.len(), tot_shares / (2 * self.num_faults + 1));
        assert_eq!(o_shares_grouped.len(), tot_shares / (2 * self.num_faults + 1));
        // Check each group has correct number of elements
        // Why self.num_faults? Is it not supposed to be 2*num_faults+1?
        //assert!(a_vec_shares_grouped.iter().all(|x| x.len() == self.num_faults));
        //assert!(b_vec_shares_grouped.iter().all(|x| x.len() == self.num_faults));
        //assert!(r_shares_grouped.iter().all(|x| x.len() == self.num_faults));
        //assert!(o_shares_grouped.iter().all(|x| x.len() == (self.num_faults * tot_shares) / (2*self.num_faults + 1)));

        let vandermonde_points: Vec<LargeField> = (2..self.num_nodes+1).into_iter().map(|x| LargeField::from(x as u64)).collect();
        let vdm_matrix = Self::vandermonde_matrix(vandermonde_points, self.num_faults); // TODO: can initialize the vdm_matrix somewhere outside to not compute it each time this gets called

        let mut o_tilde_grouped:Vec<Vec<LargeField>> = Vec::with_capacity(tot_shares / (2 * self.num_faults + 1));
        let mut zs = Vec::with_capacity(2 * self.num_faults + 1);
        
        let mut shares_party: HashMap<usize, Vec<Option<LargeField>>> = HashMap::default();
        for party in 0..self.num_nodes{
            shares_party.insert(party, Vec::with_capacity(tot_shares));
        }
        let mut share_for_party: Vec<HashMap<usize, Option<LargeField>>> = Vec::with_capacity(tot_shares);

        // Compute all the shares and store them in share_for_party[group][party]
        // Maybe this can be parallelized? 
        for i in 0..(tot_shares / (2 * self.num_faults + 1)) {
            o_tilde_grouped[i] = Self::matrix_vector_multiply(&vdm_matrix, &o_shares_grouped[i]);
            zs[i] = Vec::with_capacity(2 * self.num_faults + 1);

            let mut contains_none = false;
            for k in 1..=(2 * self.num_faults + 1) {
                if Self::contains_only_some(&a_vec_shares_grouped[i][k]) && Self::contains_only_some(&b_vec_shares_grouped[i][k]) {} else {
                    contains_none = true;
                }
            }
            if contains_none {
                // Cannot compute shares if there are bot in a/b
                for p in 1..=self.num_nodes {
                    share_for_party[i].insert(p, None);
                }
            } else {
                for k in 1..=(2 * self.num_faults + 1) {
                    let a: Vec<LargeField> = a_vec_shares_grouped[i][k].iter().map(|x| { x.unwrap() }).collect();
                    let b: Vec<LargeField> = b_vec_shares_grouped[i][k].iter().map(|x| { x.unwrap() }).collect();
                    zs[i][k] = Self::dot_product(&a, &b).add(r_shares_grouped[i][k].clone());
                }
                // Use FFTs here if possible
                let polynomial = Polynomial::new(&zs[i]); // Create polynomial from the computed zs
                // Create evaluations at roots of unity?
                // The first level evaluation should still be conducted over normal field elements, the second level evaluation can be conducted over roots of unity
                let evaluations_res 
                    = Polynomial::evaluate_fft::<FieldType>(&polynomial, 0, Some(self.num_nodes));
                if evaluations_res.is_err(){
                    log::error!("Error evaluating polynomial at roots of unity: {:?}, switching to default evaluation", evaluations_res.err());
                    for p in 0..self.num_nodes {
                        let evaluation_point = LargeField::from(p as u64);
                        let share = Self::evaluate_polynomial_from_coefficients_at_position(zs[i].clone(), evaluation_point) + o_tilde_grouped[i][p];
                        
                        shares_party.get_mut(&p).unwrap().push(Some(share));
                    }
                }
                else{
                    let evaluations = evaluations_res.unwrap();
                    for (index,share) in (0..self.num_nodes).into_iter().zip(evaluations.into_iter()){
                        shares_party.get_mut(&index).unwrap().push(Some(share));
                    }
                }
            }
        }

        // Send shares for all groups to all parties
        for (party,shares) in shares_party.into_iter(){
            let ser_shares: Vec<Option<LargeFieldSer>> = shares.into_iter().map(|share| {
                if share.is_none(){
                    return None;
                }
                else{
                    return Some(share.unwrap().to_bytes_be());
                }
            }).collect();
            // Encrypt shares before putting them in a message
            let ser_shares_bytes = bincode::serialize(&ser_shares).unwrap();
            let sec_key = self.sec_key_map.get(&party).clone().unwrap();

            let encrypted_msg = encrypt(sec_key, ser_shares_bytes);
            let prot_msg = ProtMsg::SharesL1(encrypted_msg, depth);

            let wrapper_msg = WrapperMsg::new(prot_msg, self.myid, &sec_key);
            let cancel_handler = self.net_send.send(party, wrapper_msg).await;

            self.add_cancel_handler(cancel_handler);
        }
        
        // for i in 0..(tot_shares / (2 * self.num_faults + 1)) {
        //     for p in 1..=self.num_nodes {
        //         // send share to P_p
        //         let replica = p;
        //         let mut content = serialize_group_value_option(GroupValueOption {
        //             group: i,
        //             value: share_for_party[i][&p]
        //         });
        //         let msg = Msg {
        //             content,
        //             origin: self.myid
        //         };
        //         let distribute_sharing_of_share_msg =  ProtMsg::FxShareMessage(msg.clone(), self.myid);
        //         let sec_key_for_replica = self.sec_key_map[&(replica)].clone();
        //         let wrapper_msg = WrapperMsg::new(
        //             distribute_sharing_of_share_msg.clone(),
        //             self.myid,
        //             &sec_key_for_replica.as_slice()
        //         );
        //         self.send(replica, wrapper_msg).await;
        //     }
        // }
    }

    pub async fn handle_l1_message(&mut self, enc_msg: Vec<u8>, depth: usize, sender: usize) {
        let sec_key = self.sec_key_map.get(&sender).clone().unwrap();

        let dec_msg = decrypt(sec_key, enc_msg);
        // Try deserializing the message now

        let shares_option: Result<Vec<Option<LargeFieldSer>>> = bincode::deserialize(&dec_msg);
        if shares_option.is_err() {
            log::error!("Error deserializing shares: {:?}", shares_option.err());
            return;
        }

        let shares_ser = shares_option.unwrap();
        for share in shares_ser.iter(){
            if share.is_none(){
                log::error!("Received abort message from party {}, aborting the protocol", sender);
                // TODO: trigger abort
                return;
            }
        }
        // Received message as L1 share so multiplication at this depth must be linear
        
        let shares: Vec<LargeField> = shares_ser.into_iter().map(|share| {
            return LargeField::from_bytes_be(&share.unwrap()).unwrap();
        }).collect();

        let depth_state = self.mult_state.get_single_depth_state(depth, true, shares.len());

        // At L1, the evaluation point is the point at which the polynomials have been evaluated. 
        let evaluation_point = Self::get_share_evaluation_point(sender, self.use_fft.clone(), self.roots_of_unity.clone());
        for (index, share) in shares.into_iter().enumerate(){
            depth_state.l1_shares[index].0.push(evaluation_point);
            depth_state.l1_shares[index].1.push(share);
        }
        
        depth_state.recv_share_count_l1 +=1;
        //depth_state.recv_share_count_l1 = depth_state.recv_share_count_l1.clone().add(1).into();
        let mut ser_shares = None;
        if depth_state.recv_share_count_l1.eq(&(self.num_nodes - self.num_faults)){
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
            self.broadcast(ProtMsg::SharesL2(ser_shares.unwrap())).await;
        }
        // self.received_fx_shares.entry(group).or_insert_with(Vec::new).push((LargeField::from(evaluation_point as u64), share));

        // if self.reconstruction_result.contains_key(&group) {
        //     // reconstruction was already sent to other parties for this group --> skip
        // } else {
        //     if share.is_none() {
        //         self.reconstruction_result.insert(group, None);
        //         self.distribute_reconstruction_result(group).await;
        //     } else if self.received_fx_shares.len() >= 2*self.num_faults+1 {
        //         let points = self.received_fx_shares.get(&group).unwrap().iter().map(|x| (x.0, x.1.unwrap())).collect_vec();
        //         let coefficients: Vec<LargeField> = interpolate_polynomial(points);
        //         let evaluation_result = evaluate_polynomial_from_coefficients_at_position(coefficients, LargeField::zero());
        //         self.reconstruction_result.insert(group, Some(evaluation_result));
        //         self.distribute_reconstruction_result(group).await;
        //     }
        // }
    }

    pub async fn handle_l2_message(&mut self, group_shares: Vec<LargeFieldSer>, sender: Replica, depth: usize){
        // Multiplication at this depth is of course using two levels of mult
        let depth_state = self.mult_state.get_single_depth_state(depth, true, group_shares.len());
        // At this depth, we are using roots of unity to conduct evaluation
        let evaluation_point = self.roots_of_unity.get(sender).clone().unwrap();

        for (state,group_share) in depth_state.l2_shares.iter_mut().zip(group_shares.into_iter()){
            let group_lf_share = LargeField::from_bytes_be(&group_share).unwrap();
            state.0.push(evaluation_point.clone()); // Store the evaluation point
            state.1.push(group_lf_share); // Store the share itself
        }

        depth_state.recv_share_count_l2 +=1;
        // Interpolate polynomial
        if depth_state.recv_share_count_l2 == (self.num_nodes - self.num_faults) {
            // We have enough shares to reconstruct the polynomial
            let secrets: Vec<LargeField> = depth_state.l2_shares.par_iter().map(|(indices,group_shares)|{
                let poly = Polynomial::interpolate(indices, group_shares).unwrap();
                //let secret = poly.evaluate(&LargeField::zero()); // Evaluate at zero to get the secret
                return poly.coefficients;
            }).flatten().collect();

            // Get the random sharings
            log::info!("Reconstructed {} blinded secrets at depth {}",secrets.len(), depth);
            // Subtract random sharings
            log::info!("Subtracting random sharings with length {} from reconstructed secrets at depth {}",depth_state.util_rand_sharings.len(), depth);

            if depth_state.util_rand_sharings.len() <= secrets.len(){
                log::info!("Moving on to depth {}", depth + 1);
                // Par iter from rayon not needed here because we are not doing heavy computation
                let _shares_next_depth: Vec<LargeField> 
                        = depth_state.util_rand_sharings.clone().into_iter()
                            .zip(secrets.into_iter())
                                .map(|(sharing, recon_secret)|recon_secret-sharing)
                                    .collect();

            }
            else{
                log::error!("Secrets less than number of random sharings used, this should not happen. Abandoning the protocol at depth {}",depth);
                return;
            }
        }
    }

    // pub async fn distribute_reconstruction_result(self: &mut Context, group: usize) {
    //     for p in 1..=self.num_nodes {
    //         let content = GroupValueOption {
    //             group: group,
    //             value: self.reconstruction_result[&group]
    //         };
    //         let msg = Msg {
    //             content: serialize_group_value_option(content),
    //             origin: self.myid
    //         };
    //         let m =  ProtMsg::GroupReconstructionMessage(msg.clone(), self.myid);
    //         let sec_key_for_replica = self.sec_key_map[&(p)].clone();
    //         let wrapper_msg = WrapperMsg::new(
    //             m.clone(),
    //             self.myid,
    //             &sec_key_for_replica.as_slice()
    //         );
    //         self.send(p, wrapper_msg).await;
    //     }
    // }

    // pub async fn handle_reconstruction_result_message(self: &mut Context, msg: Msg) {
    //     let content = msg.content;
    //     let deserialized_content = deserialize_group_value_option(&content);
    //     let sender: usize = msg.origin as usize;
    //     let group: usize = deserialized_content.group;
    //     let value: Option<LargeField<Stark252PrimeField>> = deserialized_content.value;

    //     self.received_reconstruction_shares.entry(group).or_insert_with(HashMap::new).insert(LargeField::from(sender as u64), value);
    //     if self.received_reconstruction_shares[&group].len() >= 2*self.num_faults + 1 && !self.Z.contains_key(&group) {
    //         let shares =  self.received_reconstruction_shares.get(&group).unwrap().iter().map(|x| (x.0.clone(), x.1.clone().unwrap())).collect_vec();
    //         let mut coefficients: Vec<LargeField<Stark252PrimeField>> = vec![LargeField::zero(); 2*self.num_faults + 1];
    //         let coeff_tmp = interpolate_polynomial(shares);
    //         for (index, value) in coeff_tmp.iter().enumerate() {
    //             coefficients[index] = *value;
    //         }

    //         self.coefficients_z.insert(group, coefficients);
    //         let hash: Vec<u8> = hash_vec_u8(self.coefficients_z[&group].clone());
    //         self.Z.insert(group.clone(), hash);

    //         // Broadcast Z[group]
    //         let content = GroupHashValueOption {
    //             group: group,
    //             value: Some(self.Z[&group].clone())
    //         };
    //         let serialized_content = serialize_group_hash_value_option(content);
    //         let msg = Msg {
    //             content: serialized_content,
    //             origin: self.myid
    //         };
    //         let distribute_sharing_of_share_msg =  ProtMsg::HashBroadcastMessage(msg, self.myid);
    //         self.broadcast_all(distribute_sharing_of_share_msg).await; // TODO: May need to invoke custom RBC here and adapt invocation of handle_Z_hash_broadcast_message. How to handle this?

    //     }
    // }

    // pub async fn handle_Z_hash_broadcast_message(self: &mut Context, msg: Msg) {
    //     let content = msg.content;
    //     let deserialized_content = deserialize_group_hash_value_option(&*content);
    //     let group: usize = deserialized_content.group;
    //     let value: Option<Vec<u8>> = deserialized_content.value;

    //     self.received_Z.entry(group).or_insert_with(Vec::new).push(value);

    //     if self.received_Z[&group].iter().any(|x| x.is_none()) || !self.received_Z[&group].windows(2).all(|w| w[0] == w[1]) {
    //         self.result.insert(group, WeakShareMultiplicationResult::FAIL);
    //     } else {
    //         if self.received_Z[&group].len() >= 2*self.num_faults + 1 {
    //             for k in 1..=2*self.num_faults + 1 {
    //                 self.cs[group][k] = Some(self.zs[group][k].sub(self.r_shares_grouped[group][k].unwrap()));
    //             }
    //         }
    //     }

    //     // TODO: add self.cs to self.result
    //     if self.result.len() == 2*self.num_faults+1 && self.result.iter().all(|x| matches!(x.1, WeakShareMultiplicationResult::FAIL) || matches!(x.1, WeakShareMultiplicationResult::SUCCESS(_, _))) {
    //         // TODO: uncomment terminate call; signature needs to be fixed
    //         // self.terminate(self.result.clone()).await;
    //     }
    // }

    pub(crate) fn group_elements_by_count<T: Clone + Send + Sync>(elements: Vec<T>, num_groups: usize) -> Vec<Vec<T>> {
        if elements.is_empty() || num_groups == 0 {
            return Vec::new();
        }
    
        let total_elements = elements.len();
        let actual_num_groups = num_groups.min(total_elements);
        let elements_per_group = (total_elements + actual_num_groups - 1) / actual_num_groups; // Ceiling division
        
        (0..actual_num_groups).into_par_iter().map(|group_idx| {
            let start_idx = group_idx * elements_per_group;
            let mut group = Vec::with_capacity(elements_per_group);
            
            for j in 0..elements_per_group {
                let idx = start_idx + j;
                if idx < total_elements {
                    group.push(elements[idx].clone());
                } else if !group.is_empty() {
                    let last = group.last().unwrap().clone();
                    group.push(last);
                }
            }
            group
        }).collect()
    }

    pub(crate) fn dot_product(
        a: &Vec<LargeField>,
        b: &Vec<LargeField>,
    ) -> LargeField {
        // Assert that the vectors have the same length
        assert_eq!(a.len(), b.len(), "Vectors must have the same length");
    
        // Compute the dot product
        a.iter()
            .zip(b.iter())
            .map(|(x, y)| *x * *y)
            .sum()
    }

    pub(crate) fn evaluate_polynomial_from_coefficients_at_position(
        coefficients: Vec<LargeField>,
        evaluation_point: LargeField,
    ) -> LargeField {
        Polynomial::new(&coefficients).evaluate(&evaluation_point)
    }

    pub(crate) fn contains_only_some<T: Send + Sync>(values: &Vec<Option<T>>) -> bool {
        values.iter().find(|value| value.is_none()).is_none()
    }

    pub fn get_share_evaluation_point(party: usize, use_fft:bool, roots_of_unity: Vec<LargeField>)-> LargeField{
        if use_fft{
            roots_of_unity.get(party).clone().unwrap().clone()
        }
        else{
            LargeField::from(party as u64)
        }
    }
}