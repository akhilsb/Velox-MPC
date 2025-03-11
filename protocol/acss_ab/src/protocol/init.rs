use std::{collections::HashMap, ops::{Mul, Add, Sub}};

use crate::Context;
use crypto::{hash::{do_hash, Hash}, aes_hash::MerkleTree, encrypt};
use lambdaworks_math::{unsigned_integer::element::UnsignedInteger, polynomial::Polynomial, traits::ByteConversion,   field::{fields::{montgomery_backed_prime_fields::MontgomeryBackendPrimeField, fft_friendly::stark_252_prime_field::MontgomeryConfigStark252PrimeField}}};
use protocol::{LargeField, LargeFieldSer};
use rand::random;
use rand_chacha::ChaCha20Rng;
use rand_core::{SeedableRng, RngCore};
use types::Replica;

impl Context{
    pub async fn init_acss_ab(&self, secrets: Vec<LargeField>){
        // Parallelize the generation of evaluation points
        let mut secret_shards: Vec<Vec<LargeField>> = secrets.chunks(self.num_threads).map(|el_vec| el_vec.to_vec()).collect();
        let mut handles = Vec::new();
        let mut evaluations;
        let nonce_evaluations;
        let mut coefficients;
        
        let blinding_poly_evaluations;
        let blinding_poly_coefficients;
        let nonce_blinding_poly_evaluations;
        if !self.use_fft{
            for shard in secret_shards.iter_mut(){
                let secrets = shard.clone();
                let handle = tokio::spawn(
                    Self::generate_evaluation_points(
                        secrets,
                        self.sec_key_map.clone(),
                        self.num_faults,
                        self.num_nodes,
                        false,
                        0u8,
                    )
                );
                handles.push(handle);
            }
    
            evaluations = Vec::new();
            coefficients = Vec::new();
                    
            for handle in handles{
                let (
                    evaluations_batch, 
                    coefficients_batch) = handle.await.unwrap();
                evaluations.extend(evaluations_batch);
                coefficients.extend(coefficients_batch);
            }
    
            // Generate nonce evaluations
            let (nonce_evaluations_ret,_nonce_coefficients) = Self::generate_evaluation_points(
                vec![LargeField::new(UnsignedInteger{
                    limbs: random()
                })],
                self.sec_key_map.clone(),
                self.num_faults,
                self.num_nodes,
                true,
                1u8
            ).await;
            nonce_evaluations = nonce_evaluations_ret[0].clone();

            // Generate the DZK proofs and commitments and utilize RBC to broadcast these proofs
            // Sample blinding polynomial
            let (blinding_poly_evaluations_vec, blinding_poly_coefficients_vec) = Self::generate_evaluation_points(
                vec![LargeField::new(UnsignedInteger{
                    limbs: random()
                })],
                self.sec_key_map.clone(),
                self.num_faults,
                self.num_nodes,
                true,
                2u8
            ).await;
            blinding_poly_evaluations = blinding_poly_evaluations_vec[0].clone();
            blinding_poly_coefficients = blinding_poly_coefficients_vec[0].clone();

            let (nonce_blinding_poly_evaluations_vec, _nonce_blinding_poly_coefficients_vec) = Self::generate_evaluation_points(
                vec![LargeField::new(UnsignedInteger{
                    limbs: random()
                })],
                self.sec_key_map.clone(),
                self.num_faults,
                self.num_nodes,
                true,
                3u8
            ).await;
            nonce_blinding_poly_evaluations = nonce_blinding_poly_evaluations_vec[0].clone();
        }
        else{
            for shard in secret_shards.iter_mut(){
                let secrets = shard.clone();
                let handle = tokio::spawn(
                    Self::generate_evaluation_points_fft(
                        secrets,
                        self.num_faults,
                        self.num_nodes
                    )
                );
                handles.push(handle);
            }
            evaluations = Vec::new();
            coefficients = Vec::new();
            for handle in handles{
                let (
                    evaluations_batch, 
                    coefficients_batch) = handle.await.unwrap();
                evaluations.extend(evaluations_batch);
                coefficients.extend(coefficients_batch);
            }

            // Generate nonce evaluations
            let (nonce_evaluations_ret,_nonce_coefficients) = Self::generate_evaluation_points_fft(
                vec![LargeField::new(UnsignedInteger{
                    limbs: random()
                })],
                self.num_faults,
                self.num_nodes,
            ).await;
            nonce_evaluations = nonce_evaluations_ret[0].clone();

            let (blinding_poly_evaluations_vec, blinding_poly_coefficients_vec) = Self::generate_evaluation_points_fft(vec![LargeField::new(UnsignedInteger{
                    limbs: random()
                })], 
                self.num_faults, 
                self.num_nodes
            ).await;
            blinding_poly_evaluations = blinding_poly_evaluations_vec[0].clone();
            blinding_poly_coefficients = blinding_poly_coefficients_vec[0].clone();

            let (nonce_blinding_evaluations_vec, _nonce_coefficients_vec) = Self::generate_evaluation_points_fft(vec![LargeField::new(UnsignedInteger{
                    limbs: random()
                })]
                , 
                self.num_faults, 
                self.num_nodes
            ).await;
            nonce_blinding_poly_evaluations = nonce_blinding_evaluations_vec[0].clone();
        }

        // Transform the shares to element wise shares
        let mut party_wise_shares: Vec<Vec<LargeFieldSer>> = Vec::new();
        let mut party_appended_shares: Vec<Vec<u8>> = Vec::new();
        for i in 1..self.num_nodes+1{
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

        let merkle_tree = MerkleTree::new(commitments.clone(), &self.hash_context);
        let share_root_comm = merkle_tree.root();

        let mut blinding_commitments = Vec::new();
        for i in 1..self.num_nodes+1{
            blinding_commitments.push(self.hash_context.hash_two( blinding_poly_evaluations[i].clone().to_bytes_be(), nonce_blinding_poly_evaluations[i].clone().to_bytes_be()));
        }

        let blinding_mt_root = MerkleTree::new(blinding_commitments.clone(), &self.hash_context).root();
        // Generate DZK coefficients
        
        
        let root_comm = self.hash_context.hash_two(share_root_comm, blinding_mt_root);
        // Convert root commitment to field element
        let root_comm_fe = LargeField::from_bytes_be(&root_comm).unwrap();
        let mut root_comm_fe_mul = root_comm_fe.clone();
        let mut dzk_coeffs = blinding_poly_coefficients.clone();
        for poly in coefficients.into_iter(){
            dzk_coeffs = dzk_coeffs.add(poly.mul(root_comm_fe_mul));
            root_comm_fe_mul = root_comm_fe_mul.mul(root_comm_fe);
        }

        // Serialize shares,commitments, and DZK polynomials
        let ser_dzk_coeffs: Vec<[u8;32]> = dzk_coeffs.coefficients.into_iter().map(|el| el.to_bytes_be()).collect();
        let broadcast_vec = (commitments, blinding_commitments, ser_dzk_coeffs, secrets.len());
        let ser_vec = bincode::serialize(&broadcast_vec).unwrap();

        let mut shares: Vec<(Replica,Option<Vec<u8>>)> = Vec::new();
        for rep in 0..self.num_nodes{
            // prepare shares
            // even need to encrypt shares
            if !self.use_fft && rep > self.num_faults{
                let shares_party = party_wise_shares[rep].clone();
                let nonce_share = nonce_evaluations[rep+1].clone().to_bytes_be();
                let blinding_nonce_share = nonce_blinding_poly_evaluations[rep+1].clone().to_bytes_be();
                
                let shares_full = (shares_party, nonce_share, blinding_nonce_share);
                let shares_ser = bincode::serialize(&shares_full).unwrap();

                let sec_key = self.sec_key_map.get(&rep).clone().unwrap();
                let enc_shares = encrypt(sec_key.as_slice(), shares_ser);
                shares.push((rep, Some(enc_shares)));
            }
        }
        // Reliably broadcast this vector
        let _rbc_status = self.inp_ctrbc.send(ser_vec).await;
        
        // Invoke AVID on vectors of shares
        // Use AVID to send the shares to parties
        let _avid_status = self.inp_avid_channel.send(shares).await;
    }

    pub async fn generate_evaluation_points(
        secrets: Vec<LargeField>, 
        sec_key_map: HashMap<Replica, Vec<u8>>,
        num_faults: usize,
        num_nodes: usize,
        is_nonce: bool,
        seed_padding: u8 // Padding for seed to generate pseudorandom values. Nonce, blinding polynomial, Blinding nonce polynomials all have their own unique nonces
    ) -> (Vec<Vec<LargeField>>, 
        Vec<Polynomial<LargeField>>
    ){
        
        // Sample the first t points on the polynomial from a Pseudorandom Function
        let mut evaluations = Vec::new();
        for secret in secrets.clone().into_iter(){
            evaluations.push(vec![secret]);
        }

        // The first evaluation is always at 0
        let mut evaluation_points = Vec::new();
        evaluation_points.push(LargeField::new(UnsignedInteger::from(0u64)));
        
        // The rest of the evaluations are using Pseudorandom functions on the secret keys between parties
        for rep in 0..num_faults+1{
            let mut sec_key = sec_key_map.get(&rep).clone().unwrap().clone();
            if is_nonce{
                sec_key.push(seed_padding);
            }
            let prf_values = Self::pseudorandom_lf(&sec_key, secrets.len());
            for (i,value) in (0..prf_values.len()).zip(prf_values.into_iter()){
                evaluations[i].push(value);
            }
            evaluation_points.push(LargeField::new(UnsignedInteger::from((rep+1) as u64)));
        }
        
        // Generate coefficients of polynomial and then evaluate the polynomial at n points
        let coefficients: Vec<Polynomial<LargeField>> = evaluations.clone().into_iter().map(|evals| {
            return Polynomial::interpolate(evaluation_points.as_slice(), evals.as_slice()).unwrap()    
        }).collect();

        // Evaluate the polynomial at n points
        for (poly_index,polynomial) in (0..coefficients.len()).into_iter().zip(coefficients.iter()){
            for index in num_faults+1..num_nodes+1{
                evaluations[poly_index].push(polynomial.evaluate(&LargeField::new(UnsignedInteger::from(index as u64))));
            }
        }
        (evaluations,coefficients)
    }

    pub async fn generate_evaluation_points_fft(
        secrets: Vec<LargeField>,
        num_faults: usize,
        num_nodes: usize,
    )-> (Vec<Vec<LargeField>>, 
        Vec<Polynomial<LargeField>>
    ){
        // For FFT evaluations, first sample coefficients of polynomial and then interpolate all n points
        let mut coefficients = Vec::new();
        for secret in secrets.clone().into_iter(){
            let mut coeffs_single_poly = Vec::new();
            coeffs_single_poly.push(secret);
            for _ in 0..num_faults{
                coeffs_single_poly.push(Self::rand_field_element());
            }
            coefficients.push(Polynomial::new(&coeffs_single_poly));
        }

        let mut evaluations = Vec::new();
        for poly_coeffs in coefficients.iter(){
            let mut poly_evaluations_fft = Polynomial::evaluate_fft::<MontgomeryBackendPrimeField<MontgomeryConfigStark252PrimeField, 4>>(poly_coeffs, 3, None).unwrap();
            // This vector has 3t+3 elements. Trim the last 2 elements
            poly_evaluations_fft.truncate(num_nodes);
            evaluations.push(poly_evaluations_fft);
        }
        (evaluations, coefficients)
    }


    pub fn rand_field_element() -> LargeField {
        let rand_big = UnsignedInteger { limbs: random() };
        LargeField::new(rand_big)
    }


    pub async fn verify_shares(&mut self, sender: Replica){
        if self.acss_ab_state.verification_status.contains_key(&sender){
            // Already verified status, abandon sharing
            return;
        }

        if !self.acss_ab_state.commitments.contains_key(&sender) || !self.acss_ab_state.shares.contains_key(&sender){
            // AVID and CTRBC did not yet terminate
            return;
        }

        let shares_full = self.acss_ab_state.shares.get(&sender).unwrap().clone();
        let shares = shares_full.0;
        let nonce_share = shares_full.1;
        let blinding_nonce_share = shares_full.2;

        let commitments_full = self.acss_ab_state.commitments.get(&sender).unwrap().clone();
        let share_commitments = commitments_full.0;
        let blinding_commitments = commitments_full.1;
        let dzk_coeffs = commitments_full.2;
        let blinding_comm_sender = blinding_commitments[sender].clone();

        // First, verify share commitments
        let mut appended_share = Vec::new();
        for share in shares.clone().into_iter(){
            appended_share.extend(share);
        }
        appended_share.extend(nonce_share);
        let comm_hash = do_hash(appended_share.as_slice());
        if comm_hash != share_commitments[sender]{
            // Invalid share commitments
            log::error!("Invalid share commitments from {}", sender);
            self.acss_ab_state.verification_status.insert(sender, false);
            return;
        }

        // Second, verify DZK proof
        let shares_ff: Vec<LargeField> = shares.into_iter().map(|el| LargeField::from_bytes_be(el.as_slice()).unwrap()).collect();
        let dzk_poly_coeffs: Vec<LargeField> = dzk_coeffs.into_iter().map(|el| LargeField::from_bytes_be(el.as_slice()).unwrap()).collect();
        let dzk_poly = Polynomial::new(dzk_poly_coeffs.as_slice());
        let dzk_point = dzk_poly.evaluate(&LargeField::new(UnsignedInteger::from((sender+1) as u64)));
        
        let share_root = MerkleTree::new(share_commitments, &self.hash_context).root();
        let blinding_root = MerkleTree::new(blinding_commitments, &self.hash_context).root();
        let root_comm = self.hash_context.hash_two(share_root, blinding_root);

        // Generate DZK point
        let root_comm_fe = LargeField::from_bytes_be(&root_comm).unwrap();
        let mut agg_shares_point = LargeField::new(UnsignedInteger::from(0u64));
        let mut root_comm_fe_mul = root_comm_fe.clone();
        for share in shares_ff{
            agg_shares_point = agg_shares_point.add(share.mul(root_comm_fe_mul));
            root_comm_fe_mul = root_comm_fe_mul.mul(root_comm_fe.clone());
        }

        let blinding_poly_share_bytes = dzk_point.sub(agg_shares_point).to_bytes_be();
        let blinding_hash = self.hash_context.hash_two(blinding_poly_share_bytes,blinding_nonce_share);
        if blinding_hash != blinding_comm_sender{
            // Invalid DZK proof
            log::error!("Invalid DZK proof from {}", sender);
            self.acss_ab_state.verification_status.insert(sender,false);
            return;
        }
        
        // If successful, add to verified list
        self.acss_ab_state.verification_status.insert(sender,true);
        // Start reliable agreement
        let _status = self.inp_ra_channel.send((sender,1,1)).await;
        self.check_termination(sender).await;
    }

    pub fn pseudorandom_lf(rng_seed: &[u8], num: usize)->Vec<LargeField>{
        let mut rng = ChaCha20Rng::from_seed(do_hash(rng_seed));
        let mut random_numbers: Vec<LargeField> = Vec::new();
        for _i in 0..num{
            let mut limbs = [0u64;4];
            for j in 0..4{
                limbs[j] = rng.next_u64();
            }
            let bigint_rand = UnsignedInteger{ 
                limbs: limbs
            };
            random_numbers.push(LargeField::new( bigint_rand));
        }
        random_numbers
    }
}