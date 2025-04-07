use std::{collections::HashMap, ops::{Add, Mul}};

use lambdaworks_math::traits::ByteConversion;
use protocol::{rand_field_element, LargeField, LargeFieldSer};
use rayon::prelude::{IntoParallelIterator, ParallelIterator};
use types::{Replica, RBCSyncMsg, SyncMsg, SyncState};
use crate::{context::Context};

impl Context{
    pub async fn init_rand_sh(&mut self, batch_size: usize, num_batches: usize){
        // Start ACSS with abort and 2t-sharing simultaneously for each batch
        for batch in 0..num_batches{
            // Create random values
            let mut rand_values = Vec::new();
            let mut zeros = Vec::new();
            for _ in 0..batch_size{
                rand_values.push(rand_field_element().to_bytes_be());
                zeros.push(LargeField::from(0 as u64).to_bytes_be());
            }

            log::info!("Initiating secret sharing in preprocessing phase for batch {}", batch);
            let status = self.acss_ab_send.send((batch,rand_values)).await;
            if status.is_err(){
                log::error!("Failed to send random values to ACSS protocol for batch {} because of error: {:?}", batch, status.err().unwrap());
            }
            let status = self.sh2t_send.send((batch,zeros)).await;
            if status.is_err(){
                log::error!("Failed to send random values to Sh2t protocol for batch {} because of error: {:?}", batch, status.err().unwrap());
            }
        }
        // Random masks for output wires
        let mut random_masks = Vec::new();
        for _ in 0..self.output_mask_size{
            random_masks.push(rand_field_element().to_bytes_be());
        }
        let avss_status = self.avss_send.send((true, Some(random_masks), None)).await;
        if avss_status.is_err(){
            log::error!("Failed to send random values to AVSS protocol {:?}", avss_status.err().unwrap());
        }
    }

    pub async fn handle_acss_term_msg(&mut self, instance: usize, sender: usize, shares: Option<Vec<LargeFieldSer>>){
        log::info!("Received ACSS shares from sender {} for batch {}", sender, instance);
        if shares.is_none(){
            log::error!("Abort ACSS protocol of dealer {} and terminate MPC", sender);
            return;
        }
        
        if self.rand_sharings_state.rand_sharings_mult.len() > 0{
            log::info!("Finished processing random sharings, ignoring ACSS and SH2t for all subsequent batches and senders: sender {}", sender);
            return;
        }

        let shares_deser: Vec<LargeField> = shares.unwrap().into_par_iter().map(|x| 
            LargeField::from_bytes_be(&x).unwrap()
        ).collect();

        if !self.rand_sharings_state.shares.contains_key(&sender){
            self.rand_sharings_state.shares.insert(sender, HashMap::default());
        }

        let shares_batches_map = self.rand_sharings_state.shares.get_mut(&sender).unwrap();
        shares_batches_map.insert(instance, shares_deser);

        self.verify_sender_termination(sender).await;
    }

    pub async fn handle_sh2t_term_msg(&mut self, instance: usize, sender: usize, shares: Option<Vec<LargeFieldSer>>){
        log::info!("Received Sh2t shares from sender {} for batch {}", sender, instance);
        if shares.is_none(){
            log::error!("Abort 2t-sharing protocol of dealer {} and terminate MPC", sender);
            return;
        }

        if self.rand_sharings_state.rand_sharings_mult.len() > 0{
            log::info!("Finished processing random sharings, ignoring ACSS and SH2t for all subsequent batches and senders: sender {}", sender);
            return;
        }
        let shares_deser: Vec<LargeField> = shares.unwrap().into_par_iter().map(|x| 
            LargeField::from_bytes_be(&x).unwrap()
        ).collect();

        if !self.rand_sharings_state.sh2t_shares.contains_key(&sender){
            self.rand_sharings_state.sh2t_shares.insert(sender, HashMap::default());
        }

        let shares_batches_map = self.rand_sharings_state.sh2t_shares.get_mut(&sender).unwrap();
        shares_batches_map.insert(instance, shares_deser);

        self.verify_sender_termination(sender).await;
    }

    pub async fn verify_sender_termination(&mut self, sender: usize){
        if !self.rand_sharings_state.shares.contains_key(&sender) || !self.rand_sharings_state.sh2t_shares.contains_key(&sender) || !self.output_mask_state.avss_shares.contains_key(&sender){
            log::debug!("ACSS, Sh2t, and AVSS not completed for sender {} for all batches", sender);
            return;
        }
        let shares_batches_map = self.rand_sharings_state.shares.get_mut(&sender).unwrap();
        let share_2t_batches_map = self.rand_sharings_state.sh2t_shares.get_mut(&sender).unwrap();
        if shares_batches_map.len() == self.tot_batches && share_2t_batches_map.len() == self.tot_batches && self.output_mask_state.avss_shares.contains_key(&sender){
            // ACSS is complete. Wait for sh2t sharings now
            log::info!("ACSS, Sh2t, and AVSS completed for sender {} for all batches", sender);
            log::info!("Batches info: {:?} {:?}", shares_batches_map.keys(),share_2t_batches_map.keys());
            self.rand_sharings_state.acss_completed_parties.insert(sender);
            let _status = self.acs_event_send.send(sender).await;
            self.verify_termination().await;
        }
    }

    pub async fn handle_acs_output(&mut self, partyset: Vec<Replica>){
        self.rand_sharings_state.acs_output.extend(partyset);
        // Check if all parties have completed ACSS and 2t-sharing
        self.verify_termination().await;
    }

    pub async fn verify_termination(&mut self){
        log::info!("Checking termination for random sharings");
        if self.rand_sharings_state.rand_sharings_mult.len() > 0{
            // Sharings already generated, return back
            return;
        } 
        if self.rand_sharings_state.acs_output.len() > 0{
            let mut flag = true;
            for party in self.rand_sharings_state.acs_output.clone().into_iter(){
                flag =  flag && self.rand_sharings_state.acss_completed_parties.contains(&party);
            }
            if flag{
                // All parties in the ACS state have completed ACSS and 2t-sharing
                // Generate random sharings
                // Vandermonde matrix
                
                let x_values: Vec<LargeField> = (2..self.num_faults+3).into_iter().map(|x| LargeField::from(x as u64)).collect();
                let vandermonde_matrix = Self::vandermonde_matrix(x_values, 2*self.num_faults+1);
                
                // Build party-accumulated share vectors
                let mut acs_indexed_share_groups: Vec<Vec<LargeField>> = Vec::new();
                let mut acs_indexed_2t_share_groups: Vec<Vec<LargeField>> = Vec::new();
                
                (0..self.tot_batches*self.per_batch).into_iter().for_each(|_|{
                    acs_indexed_share_groups.push(Vec::new());
                    acs_indexed_2t_share_groups.push(Vec::new());
                });

                for party in 0..self.num_nodes{
                    if self.rand_sharings_state.acs_output.contains(&party){
                        // First sharing
                        let mut index: usize = 0;
                        
                        let shares = self.rand_sharings_state.shares.get(&party).unwrap();
                        let shares_2t = self.rand_sharings_state.sh2t_shares.get(&party).unwrap();
                        
                        for batch in 0..self.tot_batches{
                            if !shares.contains_key(&batch){
                                log::error!("Batch {} not found in shares_batch", batch);
                            }
                            else{
                                let shares_batch = shares.get(&batch).unwrap();
                                for share in shares_batch{
                                    acs_indexed_share_groups[index].push(share.clone());
                                    index += 1;
                                }
                            }
                        }

                        index = 0;
                        for batch in 0..self.tot_batches{
                            if !shares_2t.contains_key(&batch){
                                log::error!("Batch {} not found in shares_batch for 2t shares", batch);
                            }
                            else{
                                let shares_batch = shares_2t.get(&batch).unwrap();
                                for share in shares_batch{
                                    acs_indexed_2t_share_groups[index].push(share.clone());
                                    index += 1;
                                }
                            }
                        }
                    }
                }

                // Multiply each vector with the indexed vector in the Vandermonde matrix
                let mut rand_sharings_mult: Vec<LargeField> = acs_indexed_share_groups.into_par_iter().map(|x| {
                    let res = Self::matrix_vector_multiply(&vandermonde_matrix, &x);
                    res
                }).flatten().collect();


                let rand_sharings_2t_mult: Vec<LargeField> = acs_indexed_2t_share_groups.into_par_iter().map(|x| {
                    let res = Self::matrix_vector_multiply(&vandermonde_matrix, &x);
                    res
                }).flatten().collect();


                log::info!(" Completed preprocessing and generated {} random sharings and {} random 2t sharings ", rand_sharings_mult.len(), rand_sharings_2t_mult.len());

                // Allocate 2n sharings to common coins
                let rand_sharings_coin =  rand_sharings_mult.split_off(rand_sharings_mult.len()- self.total_sharings_for_coins);
                
                // Add sharings and coins to state
                self.rand_sharings_state.rand_sharings_mult.extend(rand_sharings_mult);
                self.rand_sharings_state.rand_sharings_coin.extend(rand_sharings_coin);
                self.rand_sharings_state.rand_2t_sharings_mult.extend(rand_sharings_2t_mult);
                
                
                // Clear acss sharings now
                self.rand_sharings_state.shares.clear();
                self.rand_sharings_state.sh2t_shares.clear();

                self.generate_random_mask_shares(self.rand_sharings_state.acs_output.clone(),vandermonde_matrix).await;
                self.run_online_phase().await;
                //self.terminate("Term".to_string()).await;
            }
        }
    }

    /// Constructs the Vandermonde matrix for a given set of x-values. Note that the x-values are parties and are converted to the ith root of unity for the evaluation
    pub fn vandermonde_matrix(x_values: Vec<LargeField>, y_vals_target: usize) -> Vec<Vec<LargeField>> {
        let n = x_values.len();
        let mut matrix = vec![vec![LargeField::zero(); y_vals_target]; n];

        for (row, x) in x_values.iter().enumerate() {
            let mut value = LargeField::one();
            for col in 0..y_vals_target {
                matrix[row][col] = value.clone();
                value = value * x;
            }
        }
        matrix
    }

    pub fn matrix_vector_multiply(
        matrix: &Vec<Vec<LargeField>>,
        vector: &Vec<LargeField>,
    ) -> Vec<LargeField> {
        matrix
            .iter()
            .map(|row| {
                row.iter()
                    .zip(vector)
                    .fold(LargeField::zero(), |sum, (a, b)| sum.add(a.mul(b)))
            })
            .collect()
    }

    //Invoke this function once you terminate the protocol
    pub async fn terminate(&mut self, data: String) {
        let rbc_sync_msg = RBCSyncMsg{
            id: 1,
            msg: data,
        };

        let ser_msg = bincode::serialize(&rbc_sync_msg).unwrap();
        let cancel_handler = self
            .sync_send
            .send(
                0,
                SyncMsg {
                    sender: self.myid,
                    state: SyncState::COMPLETED,
                    value: ser_msg,
                },
            )
            .await;
        self.add_cancel_handler(cancel_handler);
    }
}