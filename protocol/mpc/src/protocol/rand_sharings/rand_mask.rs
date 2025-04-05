use std::collections::{HashMap, VecDeque};

use lambdaworks_math::traits::ByteConversion;
use protocol::{AvssShare, LargeField};
use rayon::prelude::{IntoParallelIterator, ParallelIterator};
use types::Replica;

use crate::Context;

pub struct RandomOutputMaskStruct{
    pub avss_shares: HashMap<Replica, AvssShare>,

    pub rand_sharings: VecDeque<LargeField>,
    pub recon_shares: HashMap<Replica, HashMap<Replica, Vec<LargeField>>>,
}

impl RandomOutputMaskStruct{
    pub fn new() -> Self{
        Self{
            avss_shares: HashMap::default(),

            rand_sharings: VecDeque::new(),
            recon_shares: HashMap::default(),
        }
    }
}


impl Context{
    pub async fn handle_avss_share_output(&mut self, origin: Replica, avss_share: AvssShare){
        self.output_mask_state.avss_shares.insert(origin, avss_share);
    }

    pub async fn generate_random_mask_shares(&mut self, vdm_matrix: Vec<Vec<LargeField>>){
        if self.rand_sharings_state.acs_output.len() == 0{
            return;
        }
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

    pub async fn handle_avss_share_oracle_output(&mut self, origin: Replica, share_sender: Replica, avss_share: AvssShare){
        if !self.output_mask_state.recon_shares.contains_key(&origin){
            self.output_mask_state.recon_shares.insert(origin, HashMap::default());
        }
        let share_map= self.output_mask_state.recon_shares.get_mut(&origin).unwrap();
        share_map.insert(share_sender, avss_share.0.into_iter().map(|x| LargeField::from_bytes_be(&x).unwrap()).collect::<Vec<LargeField>>());
    }
}