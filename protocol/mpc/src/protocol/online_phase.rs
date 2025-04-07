use lambdaworks_math::{traits::ByteConversion, polynomial::Polynomial};
use protocol::{LargeFieldSer, LargeField};
use types::Replica;

use crate::{Context, msg::ProtMsg};

impl Context{
    // This function will be used to run the online phase of the protocol
    pub async fn run_online_phase(&mut self) {
        // Take two random sharings, and multiply them using a random double sharing
        let mut a_shares = Vec::new();
        let mut b_shares = Vec::new();

        let tot_sharings = 2*self.num_faults+2;
        let mut combined_shares = vec![vec![];tot_sharings];

        for i in 0..tot_sharings{
            let share = self.rand_sharings_state.rand_sharings_mult.pop_front().unwrap();
            a_shares.push(vec![share.clone()]);
            combined_shares[i].push(share);
        }
        for i in 0..tot_sharings{
            let share = self.rand_sharings_state.rand_sharings_mult.pop_front().unwrap();
            b_shares.push(vec![share.clone()]);
            combined_shares[i].push(share);
        }

        self.choose_multiplication_protocol(a_shares, b_shares, 1).await;
        for (index,coup) in combined_shares.into_iter().enumerate(){
            self.reconstruct_rand_sharings(coup, index).await;
        }
    }

    pub async fn handle_mult_term_tmp(&mut self, shares: Vec<LargeField>){
        self.reconstruct_rand_sharings(shares, 5).await;
    }

    pub async fn reconstruct_rand_sharings(&mut self, shares: Vec<LargeField>, index: usize){
        // Reconstruct the output
        let output_masks_ser = shares.iter()
            .map(|x| x.to_bytes_be())
            .collect::<Vec<LargeFieldSer>>();

        self.broadcast(ProtMsg::ReconstructMultSharings(output_masks_ser, index)).await;
        // Save random masks for public recontruction after the output
    }

    pub async fn handle_reconstruct_mult_sharings(&mut self, shares: Vec<LargeFieldSer>, index: usize, sender: Replica){
        // Save shares from this sender
        if self.tmp_mult_state.contains_key(&index){
            let index_state = self.tmp_mult_state.get_mut(&index).unwrap();
            index_state.0.push(Self::get_share_evaluation_point(sender, self.use_fft, self.roots_of_unity.clone()));
            log::info!("Adding share from sender {} to index {}: {:?}", sender, index, shares);
            for i in 0..index_state.1.len(){
                index_state.1[i].push(LargeField::from_bytes_be(&shares[i]).unwrap());
            }
            if index_state.0.len() == self.num_faults+1{
                // Reconstruct these points
                let evaluation_indices = index_state.0.clone();
                let evaluations = index_state.1.clone();
                if index == 5{
                    // Reconstructed values
                    let mult_values: Vec<LargeField> = evaluations.iter().map(|evals|{
                        let poly = Polynomial::interpolate(
                            &evaluation_indices,
                            evals
                        ).unwrap();
                        return poly.evaluate(&LargeField::zero());
                    }).collect();
                    log::info!("Reconstructed multiplication value at index {}: {:?}", index, mult_values);
                }
                else{
                    let mult_value: LargeField = evaluations.iter().map(|evals|{
                        let poly = Polynomial::interpolate(
                            &evaluation_indices,
                            evals
                        ).unwrap();
                        return poly.evaluate(&LargeField::zero());
                    }).fold(LargeField::one(), |acc, x| acc*x);
                    log::info!("Reconstructed multiplication value at index {}: {:?}", index, mult_value);
                }
            }
        }
        else{
            let mut indices_vec = Vec::new();
            indices_vec.push(Self::get_share_evaluation_point(sender, self.use_fft, self.roots_of_unity.clone()));

            let mut shares_vec = vec![vec![];shares.len()];
            for i in 0..shares.len(){
                shares_vec[i].push(LargeField::from_bytes_be(&shares[i]).unwrap());
            }
            self.tmp_mult_state.insert(index, (indices_vec, shares_vec));
        }
    }
}