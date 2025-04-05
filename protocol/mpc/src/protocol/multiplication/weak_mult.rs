use crypto::{hash::{Hash}};
use lambdaworks_math::{polynomial::Polynomial};
use protocol::{LargeField};
use rayon::prelude::{ParallelIterator, IntoParallelIterator};
use types::{Replica};

use crate::{Context};

use super::mult_state::SingleDepthState;

impl Context{
    pub async fn choose_multiplication_protocol(&mut self, a_shares: Vec<Vec<LargeField>>, b_shares: Vec<Vec<LargeField>>, depth: usize){
        self.quadratic_multiplication_prot(a_shares, b_shares, depth).await;
    }

    pub async fn handle_hash_broadcast(&mut self, hash: Hash, depth: usize, lin_or_quad: bool, sender: Replica){
        if !self.mult_state.depth_share_map.contains_key(&depth){
            let single_depth_state = SingleDepthState::new(lin_or_quad);
            self.mult_state.depth_share_map.insert(depth, single_depth_state);
        }
        
        let ex_mult_state = self.mult_state.depth_share_map.get_mut(&depth).unwrap();
        ex_mult_state.recv_hash_set.insert(hash.clone());
        ex_mult_state.recv_hash_msgs.push(sender);
        self.verify_depth_mult_termination(depth).await;
    }

    pub async fn verify_depth_mult_termination(&mut self, depth: usize){
        // Now, subtract random sharings from the reconstructed secrets
        if !self.mult_state.depth_share_map.contains_key(&depth){
            return;
        }
        let mult_state = self.mult_state.depth_share_map.get(&depth).unwrap();
        if mult_state.recv_hash_set.len() == self.num_nodes-self.num_faults && mult_state.recv_hash_set.len() == 1{
            log::info!("Received 2t+1 Hashes for multiplication at depth {} with Hash {:?}, computing sharings of output gate",depth, mult_state.recv_hash_set);            
        }
        else{
            return;
        }
        let reconstructed_blinded_secrets;
        if mult_state.two_levels{
            reconstructed_blinded_secrets = mult_state.l2_shares_reconstructed.clone();
        }
        else{
            // Quadratic multiplication layer
            reconstructed_blinded_secrets = mult_state.l1_shares_reconstructed.clone();
        }
        
        // Get the random sharings
        // Subtract random sharings
        log::info!("Subtracting random sharings with length {} from reconstructed secrets {} at depth {}",mult_state.util_rand_sharings.len(), reconstructed_blinded_secrets.len(), depth);

        if mult_state.util_rand_sharings.len() <= reconstructed_blinded_secrets.len() && reconstructed_blinded_secrets.len() > 0{
            log::info!("Moving on to depth {}", depth + 1);
            // Par iter from rayon not needed here because we are not doing heavy computation
            let shares_next_depth: Vec<LargeField> 
                    = mult_state.util_rand_sharings.clone().into_iter()
                        .zip(reconstructed_blinded_secrets.into_iter())
                            .map(|(sharing, recon_secret)|recon_secret-sharing)
                                .collect();
            
            self.verf_state.add_mult_output_shares(depth, shares_next_depth.clone()); // Store the shares for the next depth
            // self.choose_multiplication_protocol(a_shares, b_shares, depth)
            // How to handle next depth wires?
            if depth == self.max_depth{
                // Trigger output reconstruction
                // Add output wires to the multiplication state as well. 
                log::info!("Multiplication terminated at depth {}, adding random masks to output wires",depth);
                // TODO: make all these addition and multiplication wires
                self.mult_state.output_layer.output_wire_shares.insert(
                    self.myid, (
                        Self::get_share_evaluation_point(self.myid, self.use_fft, self.roots_of_unity.clone())
                        ,shares_next_depth
                    )
                );
                log::info!("Starting verification of multiplications");
                // Start verification from here
                //self.delinearize_mult_tuples().await;
            }
            else if depth > self.max_depth{
                // TODO: Initiate next depth multiplication here. 
                self.handle_ex_mult_termination(depth, shares_next_depth).await;
            } 
        }
        else{
            log::error!("Secrets less than number of random sharings used, this should not happen. Abandoning the protocol at depth {}",depth);
            return;
        }
    }

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