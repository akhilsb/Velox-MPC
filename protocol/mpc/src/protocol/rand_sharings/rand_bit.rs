use std::{cmp::Ordering, ops::{Mul}};

use lambdaworks_math::{traits::ByteConversion, polynomial::Polynomial};
use protocol::{LargeFieldSer, LargeField, vandermonde_matrix, inverse_vandermonde, matrix_vector_multiply};
use rayon::prelude::{IntoParallelIterator, ParallelIterator, IndexedParallelIterator};
use types::Replica;

use crate::{Context, msg::ProtMsg};

impl Context{
    pub async fn init_rand_bit_reconstruction(&mut self){
        if !self.mix_circuit_state.rand_bit_sharings.is_empty(){
            return;
        }
        if !self.mix_circuit_state.rand_bit_recon_shares.contains_key(&self.myid){
            return;
        }
        log::info!("Initializing random bit reconstruction");
        let my_shares = self.mix_circuit_state.rand_bit_recon_shares.get(&self.myid).unwrap().clone();
        let my_shares_ser: Vec<LargeFieldSer> = my_shares.iter().map(|x| x.to_bytes_be()).collect();
        
        let prot_msg = ProtMsg::ReconstructRandBitShares(my_shares_ser);
        self.broadcast(prot_msg).await;
        self.verify_rand_bit_reconstruction().await;
    }

    pub async fn handle_reconstruct_rand_bits(&mut self, shares: Vec<LargeFieldSer>, share_sender: Replica){
        log::info!("Handling reconstruction of random bit shares from sender {}", share_sender);
        let shares: Vec<LargeField> = shares.into_iter()
            .map(|x| LargeField::from_bytes_be(&x).unwrap())
            .collect();

        let shares_len = shares.len();
        self.mix_circuit_state.rand_bit_recon_shares.insert(share_sender, shares);

        if self.mix_circuit_state.rand_bit_recon_shares.len() == self.num_faults+1{
            log::info!("Received threshold number of shares for random bit reconstruction, proceeding to reconstruct.");
            let mut indices = Vec::new();
            let mut shares_index_wise = vec![vec![];shares_len];
            
            for rep in 0..self.num_nodes{
                if self.mix_circuit_state.rand_bit_recon_shares.contains_key(&rep){
                    indices.push(Self::get_share_evaluation_point(rep, self.use_fft, self.roots_of_unity.clone()));
                    let rep_shares = self.mix_circuit_state.rand_bit_recon_shares.get(&rep).unwrap();
                    for (index, share) in rep_shares.iter().enumerate(){
                        shares_index_wise[index].push(share.clone());
                    }
                }
            }

            // generate inverse vandermonde matrix
            let vdm_matrix = vandermonde_matrix(indices);
            let inv_vdm_matrix = inverse_vandermonde(vdm_matrix);
            
            let field_div_2 = self.field_div_2.clone();
            let reconstructed_square_inverses: Vec<LargeField> = shares_index_wise.into_par_iter()
                .map(|x| {
                    let coefficients = matrix_vector_multiply(&inv_vdm_matrix, &x);
                    let secret = Polynomial::new(&coefficients).evaluate(&LargeField::from(0 as u64));
                    //return secret;
                    let sqrt_res = secret.sqrt();
                    if sqrt_res.is_none(){
                        panic!("Square root is None");
                    }
                    let (p1,p2) = sqrt_res.unwrap();
                    if p1.value().cmp(field_div_2.value()) == Ordering::Greater{
                        //return p1;
                        return p1.inv().unwrap();
                    }
                    else {
                        //return p2;
                        return p2.inv().unwrap();
                    }
                }).collect();
            
            self.mix_circuit_state.rand_bit_inverse_recon_values.extend(reconstructed_square_inverses);
            log::info!("Reconstructed random bit shares: {:?}", self.mix_circuit_state.rand_bit_inverse_recon_values);
            
            self.verify_rand_bit_reconstruction().await;
        }
    }
    
    pub async fn handle_reconstruct_rand_bits_verify(&mut self, shares: Vec<LargeFieldSer>, share_sender: Replica){
        log::info!("Handling reconstruction of random bit verify shares from sender {}", share_sender);
        let shares: Vec<LargeField> = shares.into_iter()
            .map(|x| LargeField::from_bytes_be(&x).unwrap())
            .collect();

        let shares_len = shares.len();
        self.mix_circuit_state.rand_bit_reconstruction.insert(share_sender, shares);

        if self.mix_circuit_state.rand_bit_reconstruction.len() == self.num_faults+1{
            log::info!("Received threshold number of shares for random bit reconstruction, proceeding to reconstruct.");
            let mut indices = Vec::new();
            let mut shares_index_wise = vec![vec![];shares_len];
            
            for rep in 0..self.num_nodes{
                if self.mix_circuit_state.rand_bit_reconstruction.contains_key(&rep){
                    indices.push(Self::get_share_evaluation_point(rep, self.use_fft, self.roots_of_unity.clone()));
                    let rep_shares = self.mix_circuit_state.rand_bit_reconstruction.get(&rep).unwrap();
                    for (index, share) in rep_shares.iter().enumerate(){
                        shares_index_wise[index].push(share.clone());
                    }
                }
            }

            // generate inverse vandermonde matrix
            let vdm_matrix = vandermonde_matrix(indices);
            let inv_vdm_matrix = inverse_vandermonde(vdm_matrix);
            
            let one = LargeField::one();
            let mut reconstructed_square_inverses: Vec<LargeField> = shares_index_wise.into_par_iter()
                .map(|x| {
                    let coefficients = matrix_vector_multiply(&inv_vdm_matrix, &x);
                    let secret = Polynomial::new(&coefficients).evaluate(&LargeField::from(0 as u64));
                    secret
                }).collect();
            reconstructed_square_inverses.truncate(100);
            for secret in reconstructed_square_inverses{
                log::info!("Reconstructed random bit: {}", secret);
                log::info!("One: {}",one);
                log::info!("Minus one: {}",one.inv().unwrap());
            }
        }
    }

    pub async fn verify_rand_bit_reconstruction(&mut self){
        if self.mix_circuit_state.rand_bit_inverse_recon_values.is_empty(){
            return;
        }
        if self.mix_circuit_state.rand_bit_inp_shares.is_empty(){
            return;
        }
        if !self.mix_circuit_state.rand_bit_sharings.is_empty(){
            return;
        }
        
        let reconstructed_shares = self.mix_circuit_state.rand_bit_inverse_recon_values.clone();
        let rand_bit_input_shares = self.mix_circuit_state.rand_bit_inp_shares.clone();

    
        let final_rand_bit_sharings: Vec<LargeField> = rand_bit_input_shares.into_par_iter().zip(reconstructed_shares.into_par_iter()).map(|(r,re)|{
            let mult_share = r.mul(re);
            return mult_share
        }).collect();

        self.mix_circuit_state.rand_bit_sharings.extend(final_rand_bit_sharings.clone());

        //self.mix_circuit_state.rand_bit_sharings.extend(shares_next_depth);
        self.terminate("Preprocessing".to_string()).await;
        // Start next depth and real circuit execution
        self.init_mixing().await;
    }
}