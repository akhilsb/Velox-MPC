use std::collections::{HashMap};

use consensus::reconstruct_data;
use crypto::hash::Hash;
use types::Replica;

use crate::msg::{AVIDShard};
use crate::protocol::init::construct_merkle_tree;
use crate::{AVIDState};

use crate::Context;
impl Context {
    // TODO: handle ready
    pub async fn handle_ready(self: &mut Context, 
        root_hash: Hash, 
        origin: Replica, 
        avid_shard: Option<AVIDShard>, 
        instance_id:usize, 
        ready_sender: usize
    ){
        
        if !self.avid_context.contains_key(&instance_id){
            let avid_state = AVIDState::new(origin);
            self.avid_context.insert(instance_id, avid_state);
        }
        
        let avid_context = self.avid_context.get_mut(&instance_id).unwrap();

        if avid_context.terminated{
            // RBC Already terminated, skip processing this message
            return;
        }

        let ready_senders = avid_context.readys.entry(root_hash).or_default();
        if ready_senders.contains(&ready_sender){
            return;
        }
        ready_senders.insert(ready_sender);


        if !avid_context.deliveries.contains_key(&root_hash){
            let hashmap = HashMap::default();
            avid_context.deliveries.insert(root_hash.clone(), hashmap);
        }
        let shards_map = avid_context.deliveries.get_mut(&root_hash).unwrap();
        if avid_shard.is_some(){
            let avid_shard = avid_shard.unwrap();
            if avid_shard.verify(&self.hash_context) && (avid_shard.master_proof.root() == root_hash){
                shards_map.insert(ready_sender, avid_shard);
            }
            else{
                log::error!("Received invalid shard from sender {} in instance_id {} because {} and {}",
                    ready_sender,
                    instance_id,
                    avid_shard.verify(&self.hash_context),
                    avid_shard.master_proof.root() == root_hash
                );
                return;
            }
        }

        if shards_map.len() == self.num_faults + 1 && avid_context.message.is_none(){
            // Sent ECHOs and getting a ready message for the same ECHO
            log::info!("Received enough messages for interpolating AVID message in instance {} sent by origin {}", instance_id, origin);
            let mut shards:Vec<Option<Vec<u8>>> = Vec::new();
            let mut proof_master_root = None;
            for rep in 0..self.num_nodes{   
                if shards_map.contains_key(&rep){
                    let shard = shards_map.get(&rep).unwrap();
                    if proof_master_root.is_none(){
                        proof_master_root = Some(shard.master_proof.clone());
                    }
                    shards.push(Some(shard.shard.clone()));
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
            if merkle_tree.root() == proof_master_root.unwrap().item(){
                log::info!("Reconstructed Merkle root and message successfully with validation for instance id {} from sender {}", instance_id, origin);
                let mut message = Vec::new();
                for i in 0..self.num_faults+1{
                    message.extend(shards.get(i).clone().unwrap());
                }
                avid_context.message = Some(message.clone());
            }
            else{
                log::error!("FATAL: Reconstructed Merkle root and message failed with validation for instance id {} from sender {}", instance_id, origin);
                return;
            }
        }
        if ready_senders.len() >= self.num_nodes - self.num_faults && !avid_context.terminated{
            if avid_context.message.is_some(){
                log::info!("Received n-f READY messages for AVID Instance ID {} from origin {}, terminating",instance_id, origin);
                // Terminate protocol
                let message = avid_context.message.clone().unwrap();
                avid_context.terminated = true;
                if &message[0..32] == self.zero_hash{
                    log::info!("Received dummy message, not sending to parent process");
                    return;
                }

                log::info!("Delivered message through AVID from sender {} for instance ID {}",avid_context.sender,instance_id);    
                log::info!("Trying to decrypt message with length {} with secret key of {}", message.len(), origin);
                // decrypt message

                let _sec_key = self.sec_key_map.get(&origin).unwrap().clone();
                //let msg = decrypt(sec_key.as_slice(), message);
                let status = self.out_avid.send((instance_id,avid_context.sender,Some(message))).await;
                if status.is_err(){
                    log::error!("Error sending message to parent channel {:?}", status.unwrap_err());
                }
            }
        }
    }
}
