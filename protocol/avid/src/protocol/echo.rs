use std::collections::HashSet;

use crypto::hash::Hash;
use types::{Replica};

use crate::msg::{AVIDIndexMsg, ProtMsg};
use crate::{Context, AVIDState};

impl Context {
    pub async fn handle_echo(self: &mut Context, indices: AVIDIndexMsg, echo_sender: Replica, instance_id: usize) {
        /*
        1. mp verify
        2. wait until receiving n - t echos of the same root
        3. lagrange interoplate f and m
        4. reconstruct merkle tree, verify roots match.
        5. if all pass, send ready <fi, pi>
         */
        if !self.avid_context.contains_key(&instance_id){
            let avid_state = AVIDState::new(indices.origin);
            self.avid_context.insert(instance_id, avid_state);
        }
        
        let avid_context = self.avid_context.get_mut(&instance_id).unwrap();

        if avid_context.terminated{
            // RBC Already terminated, skip processing this message
            return;
        }

        if !indices.verify_root(){
            
            log::error!("Concise root verification failed for echo sent by node {} initiated by node {}",echo_sender,indices.origin);
            return;
        
        }
        
        let echo_senders = avid_context.echos.entry(indices.concise_root).or_default();

        if echo_senders.contains(&echo_sender){
            return;
        }

        echo_senders.insert(echo_sender);
        
        let size = echo_senders.len().clone();
        if size == self.num_nodes - self.num_faults{
            log::info!("Received n-f ECHO messages for RBC Instance ID {}, sending READY message",instance_id);
            // ECHO phase is completed. Save our share and the root for later purposes and quick access. 
            let mut set_of_roots: HashSet<Hash> = HashSet::default();
            for avid_index in indices.shard_indices.clone(){
                set_of_roots.insert(avid_index.root);
            }
            avid_context.echo_roots = Some((indices.concise_root,set_of_roots));

            // Send ready message
            let avid_ready_msg = indices;
            self.handle_ready(avid_ready_msg.clone(), self.myid,instance_id).await;
            let ready_msg = ProtMsg::Ready(avid_ready_msg, instance_id);
            self.broadcast(ready_msg).await;
        }
        // Go for optimistic termination if all n shares have appeared
        else if size == self.num_nodes{
            log::info!("Received n ECHO messages for RBC Instance ID {}, terminating",instance_id);
            avid_context.terminated = true;
            // Send message to recipients and terminate
            let fragment = avid_context.fragments.clone();
            self.terminate(fragment,instance_id).await;
        }
    }
}
