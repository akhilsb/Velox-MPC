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
            //self.broadcast(ProtMsg::ReconstructCoin()).await;
        }
    }

    pub async fn handle_acss_term_msg(&mut self, instance: usize, sender: usize, shares: Option<Vec<LargeFieldSer>>){
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

        if shares_batches_map.len() == self.tot_batches{
            // ACSS is complete. Wait for sh2t sharings now
            log::info!("ACSS completed for sender {} for all batches", sender);

            self.rand_sharings_state.acss_completed_parties.insert(sender);
            self.send_term_event_to_acs_channel(sender).await;
            self.check_termination().await;
        }
    }

    pub async fn handle_sh2t_term_msg(&mut self, instance: usize, sender: usize, shares: Option<Vec<LargeFieldSer>>){
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

        if shares_batches_map.len() == self.tot_batches{
            log::info!("2t Sharing completed for sender {} for all batches", sender);
            // ACSS is complete. Wait for sh2t sharings now
            self.rand_sharings_state.sh2t_completed_parties.insert(sender);
            self.send_term_event_to_acs_channel(sender).await;
            self.check_termination().await;
        }
    }

    pub async fn send_term_event_to_acs_channel(&mut self, sender: usize){
        if self.rand_sharings_state.acss_completed_parties.contains(&sender) && self.rand_sharings_state.sh2t_completed_parties.contains(&sender){
            // Initiate ACS with this sender
            log::info!("Sender completed both 2t sharings and ACSS, initiating ACS with sender {}", sender);
            let _status = self.acs_event_send.send(sender).await;
        }
    }

    pub async fn handle_acs_output(&mut self, partyset: Vec<Replica>){
        self.rand_sharings_state.acs_output.extend(partyset);
        // Check if all parties have completed ACSS and 2t-sharing
        self.check_termination().await;
    }

    pub async fn check_termination(&mut self){
        if self.rand_sharings_state.rand_sharings_mult.len() > 0{
            // Sharings already generated, return back
            return;
        } 
        if self.rand_sharings_state.acs_output.len() > 0{
            let mut flag = true;
            for party in self.rand_sharings_state.acs_output.clone().into_iter(){
                flag =  flag && self.rand_sharings_state.acss_completed_parties.contains(&party) && self.rand_sharings_state.sh2t_completed_parties.contains(&party);
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
                        
                        for batch in 1..self.tot_batches+1{
                            let shares_batch = shares.get(&batch).unwrap();
                            for share in shares_batch{
                                acs_indexed_share_groups[index].push(share.clone());
                                index += 1;
                            }
                        }

                        index = 0;
                        for batch in 1..self.tot_batches+1{
                            let shares_batch = shares_2t.get(&batch).unwrap();
                            for share in shares_batch{
                                acs_indexed_2t_share_groups[index].push(share.clone());
                                index += 1;
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

                self.terminate("Term".to_string()).await;
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