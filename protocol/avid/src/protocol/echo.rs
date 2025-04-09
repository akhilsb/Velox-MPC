use std::collections::HashSet;

use types::{Replica, WrapperMsg};

use crate::msg::{AVIDIndexMsg, ProtMsg};
use crate::{Context, AVIDState};

impl Context {
    pub async fn handle_echo(self: &mut Context, avid_index: AVIDIndexMsg, echo_sender: Replica, instance_id: usize) {
        /*
        1. mp verify
        2. wait until receiving n - t echos of the same root
        3. lagrange interoplate f and m
        4. reconstruct merkle tree, verify roots match.
        5. if all pass, send ready <fi, pi>
         */
        if !self.avid_context.contains_key(&instance_id){
            let avid_state = AVIDState::new(avid_index.origin);
            self.avid_context.insert(instance_id, avid_state);
        }
        
        let avid_context = self.avid_context.get_mut(&instance_id).unwrap();

        if avid_context.terminated{
            // RBC Already terminated, skip processing this message
            return;
        }

        if !avid_index.proof.validate(&self.hash_context){
            log::error!("Concise root verification failed for echo sent by node {} initiated by node {}",echo_sender,avid_index.origin);
            return;
        }
        
        let master_root = avid_index.proof.root();
        let echo_senders = avid_context.echos.0.entry(master_root).or_default();
        let echo_senders_2 = avid_context.echos.1.entry(avid_index.proof.item()).or_default();
        if echo_senders.contains(&echo_sender) && echo_senders_2.contains(&echo_sender){
            return;
        }

        echo_senders.insert(echo_sender);
        echo_senders_2.insert(echo_sender);

        let size = echo_senders.len().min(echo_senders_2.len());
        if size >= self.num_nodes - self.num_faults && avid_context.agreed_root.is_none(){
            log::info!("Received n-f ECHO messages for RBC Instance ID {}, sending READY message",instance_id);
            // ECHO phase is completed. Save our share and the root for later purposes and quick access. 
            avid_context.agreed_root = Some(avid_index.proof.root());
            if avid_context.fragments.is_some(){
                // Send fragments in READY message
                let fragments = avid_context.fragments.clone().unwrap();
                let mut echo_parties: HashSet<usize> = HashSet::default();
                for avid_shard in fragments.shards.into_iter(){
                    let recipient = avid_shard.recipient;
                    let ready_msg = ProtMsg::Ready(avid_index.proof.root(), avid_shard.origin, Some(avid_shard), instance_id);
                    echo_parties.insert(recipient);

                    let sec_key = self.sec_key_map.get(&recipient).unwrap();
                    let wrapper_msg = WrapperMsg::new(ready_msg, self.myid, sec_key);
                    let _cancel_handler = self.net_send.send(recipient, wrapper_msg).await;
                    self.add_cancel_handler(_cancel_handler);
                }
                for party in 0..self.num_nodes{
                    if !echo_parties.contains(&party){
                        let ready_msg = ProtMsg::Ready(avid_index.proof.root(), avid_index.origin, None, instance_id);
                        let sec_key = self.sec_key_map.get(&party).unwrap();
                        let wrapper_msg = WrapperMsg::new(ready_msg, self.myid, sec_key);
                        let _cancel_handler = self.net_send.send(party, wrapper_msg).await;
                        self.add_cancel_handler(_cancel_handler);
                    }
                }
            }
            else{
                let ready_msg = ProtMsg::Ready(avid_index.proof.root(), avid_index.origin, None, instance_id);
                self.broadcast(ready_msg).await;
            }
        }
    }
}
