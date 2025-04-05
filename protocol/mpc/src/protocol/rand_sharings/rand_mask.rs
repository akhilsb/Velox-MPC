use std::collections::{HashMap, VecDeque, HashSet};

use lambdaworks_math::{traits::ByteConversion, polynomial::Polynomial};
use protocol::{AvssShare, LargeField, LargeFieldSer};
use rayon::prelude::{IntoParallelIterator, ParallelIterator};
use types::Replica;

use crate::{Context, msg::ProtMsg};

pub struct RandomOutputMaskStruct{
    pub avss_shares: HashMap<Replica, AvssShare>,

    pub rand_sharings: VecDeque<LargeField>,
    
    pub acs_recon_set: HashSet<Replica>,
    pub recon_shares: HashMap<Replica, HashMap<Replica, Vec<LargeField>>>,
    pub public_reconstruction_outputs: HashMap<Replica, Vec<LargeField>>
}

impl RandomOutputMaskStruct{
    pub fn new() -> Self{
        Self{
            avss_shares: HashMap::default(),

            rand_sharings: VecDeque::new(),
            acs_recon_set: HashSet::default(),

            recon_shares: HashMap::default(),
            public_reconstruction_outputs: HashMap::default()
        }
    }
}


impl Context{
    pub async fn handle_avss_share_output(&mut self, origin: Replica, avss_share: AvssShare){
        self.output_mask_state.avss_shares.insert(origin, avss_share);
    }

    pub async fn generate_random_mask_shares(&mut self, acs_recon_set: HashSet<Replica>, vdm_matrix: Vec<Vec<LargeField>>){
        if self.rand_sharings_state.acs_output.len() == 0{
            return;
        }
        self.output_mask_state.acs_recon_set.extend(acs_recon_set);
        let mut shares_accumulated: Vec<Vec<LargeField>> = vec![vec![];self.output_mask_size];
        for rep in 0..self.num_nodes{
            if self.rand_sharings_state.acs_output.contains(&rep){
                let shares = self.output_mask_state.avss_shares.get(&rep).unwrap().clone();
                for (index, share) in shares.0.iter().enumerate(){
                    shares_accumulated[index].push(LargeField::from_bytes_be(share).unwrap());
                }
            }
        }
        // Vandermonde matrix
        let random_mask_shares: Vec<LargeField> = shares_accumulated.into_par_iter().map(|x| {
            let res = Self::matrix_vector_multiply(&vdm_matrix, &x);
            res
        }).flatten().collect();
        self.output_mask_state.rand_sharings.extend(random_mask_shares);
    }

    pub async fn reconstruct_random_masks(&mut self){
        if self.rand_sharings_state.acs_output.len() == 0{
            return;
        }
        for party in 0..self.num_nodes{
            if self.rand_sharings_state.acs_output.contains(&party) && self.output_mask_state.avss_shares.contains_key(&party){
                let shares = self.output_mask_state.avss_shares.get(&party).unwrap().clone();
                let prot_msg = ProtMsg::ReconstructOutputMasks(party, shares.0, shares.1, shares.2);
                self.broadcast(prot_msg).await;
            }
        }
    }

    pub async fn handle_random_mask_shares(&mut self, share_sender: Replica, origin: Replica, shares: Vec<LargeFieldSer>, nonce: LargeFieldSer, blinding_nonce: LargeFieldSer){
        // Send request to share oracle
        if self.output_mask_state.acs_recon_set.contains(&origin){
            let _status = self.avss_send.send((false, None, Some((origin,share_sender, (shares, nonce, blinding_nonce))))).await;
        }
    }

    pub async fn handle_avss_share_oracle_output(&mut self, origin: Replica, share_sender: Replica, avss_share: AvssShare){
        if !self.output_mask_state.acs_recon_set.contains(&origin){
            // Secret already reconstructed, return from here
            return;
        }
        if !self.output_mask_state.recon_shares.contains_key(&origin){
            self.output_mask_state.recon_shares.insert(origin, HashMap::default());
        }

        let share_map= self.output_mask_state.recon_shares.get_mut(&origin).unwrap();
        share_map.insert(share_sender, avss_share.0.into_iter().map(|x| LargeField::from_bytes_be(&x).unwrap()).collect::<Vec<LargeField>>());
        if share_map.len() == self.num_faults+1{
            // Reconstruct sharings
            // While reconstructing, remove elements one by one from the acs_recon_set map
            let mut evaluation_indices = Vec::new();
            let mut evaluations = vec![vec![];self.output_mask_size];
            for party in 0..self.num_nodes{
                if share_map.contains_key(&party){
                    evaluation_indices.push(Self::get_share_evaluation_point(party, self.use_fft, self.roots_of_unity.clone()));
                    for (index, share) in share_map.get(&party).unwrap().iter().enumerate(){
                        evaluations[index].push(share.clone());
                    }
                }
            }
            // Interpolate polynomials
            let reconstructed_secrets: Vec<LargeField> = evaluations.into_par_iter().map(|x| {
                let poly = Polynomial::interpolate(&evaluation_indices, &x).unwrap();
                return poly.evaluate(&LargeField::zero());
            }).collect();
            log::info!("Reconstructed AVSS contributions of the output mask from origin {}", origin);
            self.output_mask_state.public_reconstruction_outputs.insert(origin, reconstructed_secrets);
            // Remove the origin from the acs_recon_set
            self.output_mask_state.acs_recon_set.remove(&origin);
        }
        self.verify_protocol_termination().await;
    }
    
    pub async fn verify_protocol_termination(&mut self){
        if self.output_mask_state.acs_recon_set.len() == 0{
            // Reconstruct random sharings as given by the VDM matrix
            let x_values: Vec<LargeField> = (2..self.num_faults+3).into_iter().map(|x| LargeField::from(x as u64)).collect();
            let vandermonde_matrix = Self::vandermonde_matrix(x_values, 2*self.num_faults+1);
            
            let mut rand_combined_secrets: Vec<Vec<LargeField>> = Vec::new();
            for party in self.rand_sharings_state.acs_output.clone().into_iter(){
                let avss_secrets = self.output_mask_state.public_reconstruction_outputs.get(&party).unwrap();
                if rand_combined_secrets.len() == 0{
                    for _ in 0..self.output_mask_size{
                        rand_combined_secrets.push(vec![]);
                    }
                }
                for (index, share) in avss_secrets.iter().enumerate(){
                    rand_combined_secrets[index].push(share.clone());
                }
            }
            log::info!("Reconstructed AVSS contributions of the random mask from all parties");
            // Multiply aggregated shares with Vandermonde matrix
            let rand_recon_values = rand_combined_secrets.into_par_iter().map(|x| {
                let res = Self::matrix_vector_multiply(&vandermonde_matrix, &x);
                res
            }).flatten().collect::<Vec<LargeField>>();

            // Use these reconstructed random masks to denoise the output. 
            let masked_outputs = self.mult_state.output_layer.reconstructed_masked_outputs.clone();
            if masked_outputs.is_none(){
                log::error!("Masked outputs are not available for denoising");
                return;
            }
            else{
                let masked_outputs = masked_outputs.unwrap();
                let unmasked_outputs: Vec<LargeField> = masked_outputs.into_iter().zip(rand_recon_values.into_iter()).map(|(output,mask)| output-mask).collect();
                log::info!("Unmasked output wires, protocol completed successfully with output {:?}", unmasked_outputs);
            }
        }
    }
}