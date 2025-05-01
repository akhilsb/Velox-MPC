use crate::Context;
use crypto::{hash::{do_hash, Hash}};
use lambdaworks_math::{unsigned_integer::element::UnsignedInteger};
use protocol::{LargeField, LargeFieldSer, generate_evaluation_points_fft, generate_evaluation_points, generate_evaluation_points_opt, sample_polynomials_from_prf};
use rand::random;
use types::Replica;

impl Context{
    pub async fn init_sh2t(&mut self, secrets: Vec<LargeField>, instance_id: usize){
        if !self.sh2t_state_map.contains_key(&instance_id){
            let sh2t_state = crate::Sh2tState::new();
            self.sh2t_state_map.insert(instance_id, sh2t_state);
        }

        let tot_sharings = secrets.len();

        let mut handles = Vec::new();
        let mut _indices;
        let mut evaluations;
        let nonce_evaluations;
        let mut coefficients;
        
        if !self.use_fft{
            // Generate evaluations right here
            let evaluations_prf = sample_polynomials_from_prf(
                secrets, 
                self.sec_key_map.clone(), 
                2*self.num_faults, 
                false, 
                1u8
            );
            let evaluation_prf_chunks: Vec<Vec<Vec<LargeField>>> = evaluations_prf.chunks(self.num_threads).map(|el| el.to_vec()).collect();
            for eval_prfs in evaluation_prf_chunks{
                let handle = tokio::spawn(
                    generate_evaluation_points_opt(
                        eval_prfs,
                        2*self.num_faults,
                        self.num_nodes,
                    )
                );
                handles.push(handle);
            }

            evaluations = Vec::new();
            coefficients = Vec::new();
            _indices = Vec::new();
            for party in 0..self.num_nodes{
                _indices.push(LargeField::new(UnsignedInteger::from((party+1) as u64)));
            }

            for handle in handles{
                let (
                    evaluations_batch, 
                    coefficients_batch) = handle.await.unwrap();
                evaluations.extend(evaluations_batch);
                coefficients.extend(coefficients_batch);
            }
            // Generate nonce evaluations
            let evaluations_nonce_prf = sample_polynomials_from_prf(
                vec![LargeField::new(UnsignedInteger{
                    limbs: random()
                })], 
                self.sec_key_map.clone(), 
                2*self.num_faults, 
                true, 
                1u8
            );
            let (nonce_evaluations_ret,_nonce_coefficients) = generate_evaluation_points(
                evaluations_nonce_prf,
                2*self.num_faults,
                self.num_nodes
            ).await;
            nonce_evaluations = nonce_evaluations_ret[0].clone();
        }
        else{
            // Parallelize the generation of evaluation points
            let mut secret_shards: Vec<Vec<LargeField>> = secrets.chunks(self.num_threads).map(|el_vec| el_vec.to_vec()).collect();
            for shard in secret_shards.iter_mut(){
                let secrets = shard.clone();
                let handle = tokio::spawn(
                    generate_evaluation_points_fft(
                        secrets,
                        2*self.num_faults-1,
                        self.num_nodes
                    )
                );
                handles.push(handle);
            }
            evaluations = Vec::new();
            coefficients = Vec::new();
            _indices = self.roots_of_unity.clone();

            for handle in handles{
                let (
                    evaluations_batch, 
                    coefficients_batch) = handle.await.unwrap();
                evaluations.extend(evaluations_batch);
                coefficients.extend(coefficients_batch);
            }

            // Generate nonce evaluations
            let (nonce_evaluations_ret,_nonce_coefficients) = generate_evaluation_points_fft(
                vec![LargeField::new(UnsignedInteger{
                    limbs: random()
                })],
                2*self.num_faults-1,
                self.num_nodes,
            ).await;
            nonce_evaluations = nonce_evaluations_ret[0].clone();
        }
        // Transform the shares to element wise shares
        let mut party_wise_shares: Vec<Vec<LargeFieldSer>> = Vec::new();
        let mut party_appended_shares: Vec<Vec<u8>> = Vec::new();
        for i in 0..self.num_nodes{
            let mut party_shares = Vec::new();
            let mut appended_share = Vec::new();
            for j in 0..evaluations.len(){
                party_shares.push(evaluations[j][i].clone().to_bytes_be());
                appended_share.extend(evaluations[j][i].clone().to_bytes_be());
            }
            party_wise_shares.push(party_shares);

            // Append nonce shares to shares for generating commitment
            appended_share.extend(nonce_evaluations[i].clone().to_bytes_be());
            party_appended_shares.push(appended_share);
        }

        // Generate Commitments here
        // There should be $n$ commitments overall
        let commitments: Vec<Hash> = party_appended_shares.into_iter().map(|share| {
            do_hash(&share)
        }).collect();

        let broadcast_vec = (commitments, tot_sharings);
        let inst_broadcast_vec = (instance_id, broadcast_vec);
        let ser_vec = bincode::serialize(&inst_broadcast_vec).unwrap();

        let mut shares: Vec<(Replica,Option<Vec<u8>>)> = Vec::new();
        for rep in 0..self.num_nodes{
            // prepare shares
            if (self.use_fft) || (!self.use_fft && rep >= 2*self.num_faults){
                let shares_party = party_wise_shares[rep].clone();
                let nonce_share = nonce_evaluations[rep].clone().to_bytes_be();
                
                let shares_full = (shares_party, nonce_share);
                let inst_shares_full = (instance_id, shares_full);
                let shares_ser = bincode::serialize(&inst_shares_full).unwrap();

                //let enc_shares = encrypt(sec_key.as_slice(), shares_ser);
                shares.push((rep, Some(shares_ser)));
            }
        }
        // Reliably broadcast this vector
        let _rbc_status = self.inp_ctrbc.send(ser_vec).await;
        
        // Invoke AVID on vectors of shares
        // Use AVID to send the shares to parties
        let _avid_status = self.inp_avid_channel.send(shares).await;
    }

    pub async fn verify_shares(&mut self, sender: Replica, instance_id: usize){
        if !self.sh2t_state_map.contains_key(&instance_id) {
            let sh2t_state = crate::Sh2tState::new();
            self.sh2t_state_map.insert(instance_id, sh2t_state);
        }
        let sh2t_state = self.sh2t_state_map.get_mut(&instance_id).unwrap();

        if sh2t_state.verification_status.contains_key(&sender){
            // Already verified status, abandon sharing
            return;
        }

        if !sh2t_state.commitments.contains_key(&sender) || !sh2t_state.shares.contains_key(&sender){
            // AVID and CTRBC did not yet terminate
            return;
        }

        let shares_full = sh2t_state.shares.get(&sender).unwrap().clone();
        let shares = shares_full.0;
        let nonce_share = shares_full.1;

        let commitments_full = sh2t_state.commitments.get(&sender).unwrap().clone();
        let share_commitments = commitments_full;
        
        // First, verify share commitments
        let mut appended_share = Vec::new();
        for share in shares.clone().into_iter(){
            appended_share.extend(share);
        }
        appended_share.extend(nonce_share);
        let comm_hash = do_hash(appended_share.as_slice());
        if comm_hash != share_commitments[self.myid]{
            // Invalid share commitments
            log::error!("Invalid share commitments from {}", sender);
            sh2t_state.verification_status.insert(sender, false);
            return;
        }
        
        log::info!("Share from {} verified", sender);
        // If successful, add to verified list
        sh2t_state.verification_status.insert(sender,true);
        // Start reliable agreement
        let _status = self.inp_ra_channel.send((sender,1,instance_id)).await;
        self.check_termination(sender, instance_id).await;
    }
}