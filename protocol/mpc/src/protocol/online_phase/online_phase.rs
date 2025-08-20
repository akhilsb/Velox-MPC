use std::collections::{HashMap};

use protocol::{LargeField};
use crate::{Context};

impl Context{
    // This function will be used to run the online phase of the protocol
    pub async fn init_random_shared_bits_preparation(&mut self) {
        // Take two random sharings, and multiply them using a random double sharing
        let a_shares = self.rand_sharings_state.rand_sharings_inputs.0.clone().into_iter()
            .map(|x| vec![x]).collect();
        let b_shares = self.rand_sharings_state.rand_sharings_inputs.1.clone().into_iter()
        .map(|x| vec![x]).collect();

        self.rand_sharings_state.rand_sharings_inputs.0.clear();
        self.rand_sharings_state.rand_sharings_inputs.1.clear();

        self.choose_multiplication_protocol(a_shares, b_shares, self.preprocessing_mult_depth).await;
    }

    pub async fn init_mixing(&mut self){
        // First find input wires. 
        let mut input_sharings: Vec<LargeField> = self.mix_circuit_state.input_sharings.clone();
        if input_sharings.len() < self.k_value {
            log::error!("Not enough input sharings for mixing. Expected at least {}, got {}", self.k_value, input_sharings.len());
            return;
        }
        input_sharings.truncate(self.k_value);
        // Now we have the input wires.
        self.mix_circuit_state.wire_sharings.insert(1, input_sharings);
        self.init_butterfly_mixing_level(1).await;
    }

    pub async fn verify_mixing_level_termination(&mut self, depth: usize){
        // We have the output sharings of wires of a depth here
        // If either of these parts are not present, exit this function right here
        if !self.mix_circuit_state.wire_pairs.contains_key(&depth) || !self.mix_circuit_state.mult_result.contains_key(&depth){
            return;
        }
        if self.mix_circuit_state.wire_sharings.contains_key(&(depth+1)){
            // This phase has already been processed. Remember the idempotence rule. 
            return;
        }
        let multiplication_result = self.mix_circuit_state.mult_result.get(&depth).unwrap().clone();
        let wire_pairs = self.mix_circuit_state.wire_pairs.get(&depth).unwrap().clone();
        
        // We have one multiplication result for each wire pair. 
        // Remember each wire pair will have one corresponding multiplication result. 
        if multiplication_result.len() != wire_pairs.len(){
            log::error!("Multiplication result and wire pairs lengths do not match at depth {}", depth);
            return;
        }

        let two_inverse = self.mix_circuit_state.two_inverse.clone();
        // Zip wire pairs and multiplication results
        let next_depth_wires: Vec<LargeField> = wire_pairs.into_iter().zip(multiplication_result.into_iter()).map(|(wirepair, mult_result)|{
            let sum = wirepair.0 + wirepair.1;
            
            let wire1 = (sum.clone() + mult_result.clone())*two_inverse.clone();
            let wire2 = (sum - mult_result)*two_inverse.clone();

            return vec![wire1, wire2];
        }).flatten().collect();
        if depth == self.max_depth{
            // Trigger output reconstruction
            // Add output wires to the multiplication state as well. 
            log::info!("Multiplication terminated at depth {}, adding random masks to output wires",depth);
            // TODO: make all these addition and multiplication wires
            self.mult_state.output_layer.output_shares = Some((
                Self::get_share_evaluation_point(self.myid, self.use_fft, self.roots_of_unity.clone())
                ,next_depth_wires
            ));
            self.terminate("Online".to_string(), vec![]).await;
            log::info!("Starting verification of multiplications");
            // Start verification from here
            self.delinearize_mult_tuples().await;
        }
        else{
            let next_depth = depth+1;
            self.mix_circuit_state.wire_sharings.insert(next_depth, next_depth_wires);
            // Start next depth of the circuit
            self.init_butterfly_mixing_level(next_depth).await;
        }
    }

    pub async fn init_butterfly_mixing_level(&mut self, depth: usize){
        // First, find the wires for this depth
        if !self.mix_circuit_state.wire_sharings.contains_key(&depth){
            return;
        }
        let wires = self.mix_circuit_state.wire_sharings.get(&depth).unwrap().clone();
        let mut wire_index_map: HashMap<usize, LargeField> = HashMap::default();
        wires.into_iter().enumerate().for_each(|(i,wire)|{
            wire_index_map.insert(i,wire);
        });

        // Parse the map into a butterfly pattern
        let mut wire_pairs: Vec<(LargeField, LargeField)> = Vec::new();
        let log_k = self.log_k;
        let log_switch_index = ((log_k - (depth % log_k)) % log_k) as u32;

        let switch_index = usize::pow(2, log_switch_index);
        for i in 0..self.k_value{
            if wire_index_map.contains_key(&i) && wire_index_map.contains_key(&(i+switch_index)){
                let wire1 = wire_index_map.get(&i).unwrap().clone();
                let wire2 = wire_index_map.get(&(i+switch_index)).unwrap().clone();

                wire_pairs.push((wire1, wire2));
                wire_index_map.remove(&i);
                wire_index_map.remove(&(i+switch_index));
            }
        }

        log::info!("Wire pairs created at depth {} with pair map length : {} and remaining unpaired indices: {}", depth, wire_pairs.len(), wire_index_map.len());
        // Save the wire pairs for this depth
        self.mix_circuit_state.wire_pairs.insert(depth, wire_pairs.clone());
        // Pair wirepairs with random bit sharings and prepare multiplication

        let wire_pair_difference: Vec<Vec<LargeField>> = wire_pairs.into_iter().map(|(w1,w2)| vec![w1-w2]).collect();
        let rand_sharings: Vec<Vec<LargeField>> = (0..wire_pair_difference.len()).into_iter().map(|_| vec![self.mix_circuit_state.rand_bit_sharings.pop_front().unwrap()]).collect();

        // Initialize multiplication of both these vector of vectors. 
        self.choose_multiplication_protocol(wire_pair_difference, rand_sharings, depth).await;
        Box::pin(self.verify_mixing_level_termination(depth)).await;
    }

    // pub async fn handle_mult_term_tmp(&mut self, shares: Vec<LargeField>){
    //     self.reconstruct_rand_sharings(shares, 5).await;
    // }

    // pub async fn reconstruct_rand_sharings(&mut self, shares: Vec<LargeField>, index: usize){
    //     // Reconstruct the output
    //     let output_masks_ser = shares.iter()
    //         .map(|x| x.to_bytes_be())
    //         .collect::<Vec<LargeFieldSer>>();

    //     self.broadcast(ProtMsg::ReconstructMultSharings(output_masks_ser, index)).await;
    //     // Save random masks for public recontruction after the output
    // }

    // pub async fn handle_reconstruct_mult_sharings(&mut self, shares: Vec<LargeFieldSer>, index: usize, sender: Replica){
    //     // Save shares from this sender
    //     if self.tmp_mult_state.contains_key(&index){
    //         let index_state = self.tmp_mult_state.get_mut(&index).unwrap();
    //         index_state.0.push(Self::get_share_evaluation_point(sender, self.use_fft, self.roots_of_unity.clone()));
    //         log::info!("Adding share from sender {} to index {}: {:?}", sender, index, shares);
    //         for i in 0..index_state.1.len(){
    //             index_state.1[i].push(LargeField::from_bytes_be(&shares[i]).unwrap());
    //         }
    //         if index_state.0.len() == self.num_faults+1{
    //             // Reconstruct these points
    //             let evaluation_indices = index_state.0.clone();
    //             let evaluations = index_state.1.clone();
    //             if index == 5{
    //                 // Reconstructed values
    //                 let mult_values: Vec<LargeField> = evaluations.iter().map(|evals|{
    //                     let poly = Polynomial::interpolate(
    //                         &evaluation_indices,
    //                         evals
    //                     ).unwrap();
    //                     return poly.evaluate(&LargeField::zero());
    //                 }).collect();
    //                 log::info!("Reconstructed multiplication value at index {}: {:?}", index, mult_values);
    //             }
    //             else{
    //                 let mult_value: LargeField = evaluations.iter().map(|evals|{
    //                     let poly = Polynomial::interpolate(
    //                         &evaluation_indices,
    //                         evals
    //                     ).unwrap();
    //                     return poly.evaluate(&LargeField::zero());
    //                 }).fold(LargeField::one(), |acc, x| acc*x);
    //                 log::info!("Reconstructed multiplication value at index {}: {:?}", index, mult_value);
    //             }
    //         }
    //     }
    //     else{
    //         let mut indices_vec = Vec::new();
    //         indices_vec.push(Self::get_share_evaluation_point(sender, self.use_fft, self.roots_of_unity.clone()));

    //         let mut shares_vec = vec![vec![];shares.len()];
    //         for i in 0..shares.len(){
    //             shares_vec[i].push(LargeField::from_bytes_be(&shares[i]).unwrap());
    //         }
    //         self.tmp_mult_state.insert(index, (indices_vec, shares_vec));
    //     }
    // }
}