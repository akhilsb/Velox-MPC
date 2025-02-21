use std::{collections::HashMap, ops::{Mul, Add}};

use crate::Context;
use crypto::{hash::{do_hash, Hash}, aes_hash::MerkleTree, encrypt};
use lambdaworks_math::{unsigned_integer::element::UnsignedInteger, polynomial::Polynomial, traits::ByteConversion};
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

        let mut evaluations = Vec::new();
        let mut coefficients = Vec::new();
                
        for handle in handles{
            let (
                evaluations_batch, 
                coefficients_batch) = handle.await.unwrap();
            evaluations.extend(evaluations_batch);
            coefficients.extend(coefficients_batch);
        }

        // Generate nonce evaluations
        let (nonce_evaluations,_nonce_coefficients) = Self::generate_evaluation_points(
            vec![LargeField::new(UnsignedInteger{
                limbs: random()
            })],
            self.sec_key_map.clone(),
            self.num_faults,
            self.num_nodes,
            true,
            1u8
        ).await;
        let nonce_evaluations = nonce_evaluations[0].clone();
        //let nonce_coefficients = nonce_coefficients[0].clone();
        
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
        
        // Generate the DZK proofs and commitments and utilize RBC to broadcast these proofs
        // Sample blinding polynomial
        let (blinding_poly_evaluations, blinding_poly_coefficients) = Self::generate_evaluation_points(
            vec![LargeField::new(UnsignedInteger{
                limbs: random()
            })],
            self.sec_key_map.clone(),
            self.num_faults,
            self.num_nodes,
            true,
            2u8
        ).await;
        let blinding_poly_evaluations = blinding_poly_evaluations[0].clone();
        let blinding_poly_coefficients = blinding_poly_coefficients[0].clone();

        let (nonce_blinding_poly_evaluations, _nonce_blinding_poly_coefficients) = Self::generate_evaluation_points(
            vec![LargeField::new(UnsignedInteger{
                limbs: random()
            })],
            self.sec_key_map.clone(),
            self.num_faults,
            self.num_nodes,
            true,
            3u8
        ).await;
        let nonce_blinding_poly_evaluations = nonce_blinding_poly_evaluations[0].clone();

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
        let broadcast_vec = (commitments, blinding_commitments, ser_dzk_coeffs);
        let ser_vec = bincode::serialize(&broadcast_vec).unwrap();

        let mut shares: Vec<(Replica,Option<Vec<u8>>)> = Vec::new();
        for rep in 0..self.num_nodes{
            // prepare shares
            // even need to encrypt shares
            if rep > self.num_faults{
                let shares_party = party_wise_shares[rep].clone();
                let nonce_share = nonce_evaluations[rep+1].clone().to_bytes_be();
                let blinding_share = blinding_poly_evaluations[rep+1].clone().to_bytes_be();
                
                let shares_full = (shares_party, nonce_share, blinding_share);
                let shares_ser = bincode::serialize(&shares_full).unwrap();

                let sec_key = self.sec_key_map.get(&rep).clone().unwrap();
                let enc_shares = encrypt(sec_key.as_slice(), shares_ser);
                shares.push((rep, Some(enc_shares)));
            }
            else {
                shares.push((rep, None));
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