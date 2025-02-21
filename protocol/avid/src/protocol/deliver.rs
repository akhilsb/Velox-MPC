use consensus::reconstruct_data;
use types::{Replica};

use crate::{Context, msg::{AVIDShard}, AVIDState};

use super::init::construct_merkle_tree;

impl Context{
    pub async fn handle_deliver(self: &mut Context, avid_shard: AVIDShard, origin: Replica, share_sender: Replica, instance_id: usize){
        
        if !self.avid_context.contains_key(&instance_id){
            let avid_state = AVIDState::new(origin);
            self.avid_context.insert(instance_id, avid_state);
        }
        
        let avid_context = self.avid_context.get_mut(&instance_id).unwrap();

        let avid_state = avid_context.deliveries.entry(avid_shard.proof.root()).or_default();
        if avid_shard.verify(&self.hash_context){
            avid_state.insert(share_sender, avid_shard);
        }
        
        if avid_context.terminated && avid_state.len() == self.num_faults + 1{
            // Reconstruct and verify root
            // Reconstruct the entire Merkle tree
            let mut shards:Vec<Option<Vec<u8>>> = Vec::new();
            for rep in 0..self.num_nodes{
                
                if avid_state.contains_key(&rep){
                    shards.push(Some(avid_state.get(&rep).unwrap().shard.clone()));
                }

                else{
                    shards.push(None);
                }
            }

            let status = reconstruct_data(&mut shards, self.num_faults+1 , 2*self.num_faults);
            
            if status.is_err(){
                log::error!("FATAL: Error in Lagrange interpolation {}",status.err().unwrap());
                // Do something else here
                return;
            }

            let shards:Vec<Vec<u8>> = shards.into_iter().map(| opt | opt.unwrap()).collect();
            // Reconstruct Merkle Root
            let merkle_tree = construct_merkle_tree(shards.clone(), &self.hash_context);
            if avid_context.echo_roots.clone().unwrap().1.contains(&merkle_tree.root()) {
                let mut message = Vec::new();
                for i in 0..self.num_faults+1{
                    message.extend(shards.get(i).clone().unwrap());
                }
                avid_context.message = Some(message.clone());
                log::info!("Delivered message through AVID from sender {} for instance ID {}",avid_context.sender,instance_id);
                let status = self.out_avid.send((avid_context.sender,Some(message))).await;
                if status.is_err(){
                    log::error!("Error sending message to parent channel {:?}", status.unwrap_err());
                }
            }
            else{
                // Do something else
                log::error!("Message's merkle root does not match broadcasted root for instance ID {}. Exiting",instance_id);
                let status = self.out_avid.send((avid_context.sender,None)).await;
                if status.is_err(){
                    log::error!("Error sending message to parent channel {:?}", status.unwrap_err());
                }
            }
        }
    }
}