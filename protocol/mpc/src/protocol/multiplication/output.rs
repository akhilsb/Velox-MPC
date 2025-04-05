use crypto::hash::do_hash;
use lambdaworks_math::traits::ByteConversion;
use protocol::{LargeFieldSer, LargeField};
use types::Replica;

use crate::{Context, msg::ProtMsg};

impl Context{
    // Last layer of the protocol
    pub async fn reconstruct_output(&mut self){
        if !self.mult_state.output_layer.output_wire_shares.contains_key(&self.myid){
            log::error!("Output layer shares are missing, abandoning protocol");
            return;
        }
        
        let mut output_wire_shares = self.mult_state.output_layer.output_wire_shares.get(&self.myid).unwrap().clone().1;
        // Add random masks
        let mut random_mask_shares = Vec::with_capacity(output_wire_shares.len());
        for output_wire_share in output_wire_shares.iter_mut(){
            if self.rand_sharings_state.rand_sharings_mult.is_empty(){
                log::error!("Not enough random sharings for mask reconstruction, abandoning the protocol");
                return;
            }
            let random_mask_share = self.rand_sharings_state.rand_sharings_mult.pop_front().unwrap();
            *output_wire_share += random_mask_share.clone();
            random_mask_shares.push(random_mask_share);
        }
        // Reconstruct the output
        let output_masks_ser = output_wire_shares.iter()
            .map(|x| x.to_bytes_be())
            .collect::<Vec<LargeFieldSer>>();
        

        self.broadcast(ProtMsg::ReconstructMaskedOutput(output_masks_ser)).await;
        // Save random masks for public recontruction after the output
    }

    pub async fn handle_reconstruct_masked_output(&mut self, ser_shares: Vec<LargeFieldSer>, sender:Replica){
        log::info!("Handling reconstruct masked output shares from sender {}", sender);
        // Deserialize shares
        let shares_lf: Vec<LargeField> = ser_shares.into_iter().map(|x| LargeField::from_bytes_be(&x).unwrap()).collect();
        let evaluation_point = Self::get_share_evaluation_point(sender,self.use_fft, self.roots_of_unity.clone());
        
        self.mult_state.output_layer.output_wire_shares.insert(sender, (evaluation_point, shares_lf));

        if self.mult_state.output_layer.output_wire_shares.len() == self.num_nodes - self.num_faults{
            // Reconstruct output
            let mut evaluation_points = Vec::with_capacity(self.num_nodes);
            let mut evaluations = Vec::new();
            for i in 0..self.num_nodes{
                if self.mult_state.output_layer.output_wire_shares.contains_key(&i){
                    // Evaluations and evaluation points
                    let (evaluation_point, evaluation) = self.mult_state.output_layer.output_wire_shares.get(&i).unwrap().clone();
                    evaluation_points.push(evaluation_point);
                    if evaluations.len()  == 0{
                        for _ in 0..evaluation.len(){
                            evaluations.push(Vec::new());
                        }
                    }
                    for (index,eval) in evaluation.into_iter().enumerate(){
                        evaluations[index].push(eval);
                    }
                }
            }
            // Reconstruct the outputs
            let verification_result = Self::check_if_all_points_lie_on_degree_x_polynomial(evaluation_points, evaluations, self.num_faults+1);
            if verification_result.0{
                let polys = verification_result.1.unwrap();
                // Output wires reconstructed
                log::info!("Masked output wires successfully reconstructed, shares are on a degree-t polynomial");
                let outputs_recon = polys.iter().map(|poly|poly.evaluate(&LargeField::zero())).collect::<Vec<LargeField>>();
                self.mult_state.output_layer.reconstructed_masked_outputs = Some(outputs_recon.clone());
                // Broadcast using a CTRBC channel
                let mut broadcast_output = Vec::new();
                broadcast_output.push(1u8);
                for output in outputs_recon.iter(){
                    broadcast_output.extend(output.to_bytes_be());
                }
                let _status = self.ctrbc_event_send.send(broadcast_output).await;
            }
            else{
                log::error!("Output reconstruction failed, shares not on a degree-t polynomial");
                return;
            }
        }
    }

    pub async fn handle_output_delivery_ctrbc(&mut self, _instance_id: usize, sender: Replica, output_value: Vec<u8>){
        log::info!("Received CTRBC output from party {}",sender);
        if output_value.len() == 0{
            log::error!("Received empty CTRBC output from party {}",sender);
            return;
        }
        let success = output_value[0] == 1u8;
        if success {
            log::info!("Party {} successfully reconstructed output wires", sender);
            let hash_val = do_hash(&output_value);
            self.mult_state.output_layer.broadcasted_masked_outputs.insert(hash_val);
            self.mult_state.output_layer.num_parties_broadcasted += 1;

            if self.mult_state.output_layer.num_parties_broadcasted == self.num_nodes - self.num_faults && self.mult_state.output_layer.broadcasted_masked_outputs.len() == 1{
                // Reconstruct the output
                log::info!("All parties broadcasted the same output, MPC is a success. Reconstructing the random masks");
                // Reconstruct random masks from here
            }
            else{
                log::error!("Party {} broadcasted output, waiting for others", sender);
            }
        }
        else {
            log::info!("Party {} aborted the protocol", sender);
            self.mult_state.output_layer.num_parties_aborted +=1;
            if self.mult_state.output_layer.num_parties_aborted == self.num_faults+1{
                log::error!("t+1 parties aborted the protocol, aborting the protocol");
                return;
            }
        }
    }
}