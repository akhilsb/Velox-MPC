use std::collections::{HashMap, HashSet};

use consensus::get_shards;
use crypto::{
    aes_hash::{MerkleTree, HashState},
    hash::{do_hash, Hash},
};
use types::{WrapperMsg, Replica};

use crate::{Context, msg::{AVIDMsg, AVIDShard}, AVIDState};
use crate::{ProtMsg};
use network::{plaintcp::CancelHandler, Acknowledgement};

impl Context {
    // Dealer sending message to everybody
    pub async fn start_init(self: &mut Context, msgs:Vec<(Replica,Vec<u8>)>, instance_id:usize) {
        // First encrypt messages
        let msg_set: Vec<Replica> = msgs.iter().map(|(x,_y)| *x).collect();
        let hash_set: HashSet<Replica> = HashSet::from_iter(msg_set.into_iter());
        let mut filled_msg_vec = Vec::new();
        for party in 0..self.num_nodes{
            if !hash_set.contains(&party){
                filled_msg_vec.push((party,self.zero_hash.clone().to_vec()));
            }
        }
        filled_msg_vec.extend(msgs);
        // let encrypted_msgs = msgs.into_iter().map(|(replica,msg)|{
        //     let secret_key = self.sec_key_map.get(&replica).unwrap();
        //     let encrypted_msg = encrypt(secret_key, msg);
        //     return (replica, encrypted_msg);            
        // });
        // Each element of the vector is an AVID for sending a message to a single replica
        let mut avid_tree: Vec<(Replica,Vec<Vec<u8>>,MerkleTree)> = Vec::new(); 
        let mut roots_agg: Vec<Hash> = Vec::new();
        
        for msg in filled_msg_vec{
            // Get encrypted text itself
            let shards = get_shards(msg.1, self.num_faults+1, 2*self.num_faults);
            let merkle_tree = construct_merkle_tree(shards.clone(),&self.hash_context);
            roots_agg.push(merkle_tree.root());
            avid_tree.push((msg.0,shards,merkle_tree));
        }
        
        let master_mt = MerkleTree::new(roots_agg, &self.hash_context);
        let mut party_wise_share_map: HashMap<usize, Vec<AVIDShard>> = HashMap::default();
        for party in 0..self.num_nodes{
            party_wise_share_map.insert(party, Vec::new());
        }
        for (index,tuple) in avid_tree.into_iter().enumerate(){
            for (party,fragment) in (0..self.num_nodes).into_iter().zip(tuple.1.into_iter()){
                let avid_shard = AVIDShard{
                    id: instance_id,
                    origin: self.myid,
                    recipient: tuple.0.clone(),
                    shard: fragment,
                    proof: tuple.2.gen_proof(party),
                    master_proof: master_mt.gen_proof(index),
                };
                party_wise_share_map.get_mut(&party).unwrap().push(avid_shard);
            }
        }
        
        let concise_root = master_mt.root();
        let sec_key_map = self.sec_key_map.clone();
        for (replica, sec_key) in sec_key_map.into_iter() {
            // TODO: Encryption
            let avid_shards = party_wise_share_map.get(&replica).unwrap().clone();
            
            let avid_msg = AVIDMsg {
                shards: avid_shards,
                origin: self.myid,
                concise_root: concise_root.clone()
            };
            
            let protocol_msg = ProtMsg::Init(avid_msg, instance_id);
            let wrapper_msg = WrapperMsg::new(protocol_msg.clone(), self.myid, &sec_key.as_slice());
            let cancel_handler: CancelHandler<Acknowledgement> = self.net_send.send(replica, wrapper_msg).await;
            self.add_cancel_handler(cancel_handler);
        }
    }

    pub async fn handle_init(self: &mut Context, msg: AVIDMsg, instance_id:usize) {
        
        if !msg.verify_mr_proofs(&self.hash_context) {
            log::error!(
                "Invalid Merkle Proof sent by node {}, abandoning AVID instance",
                msg.origin
            );
            return;
        }

        log::debug!(
            "Received Init message {:?} from node {}.",
            msg.shards,
            msg.origin,
        );

        if !self.avid_context.contains_key(&instance_id){
            self.avid_context.insert(instance_id, AVIDState::new(msg.origin));
        }
        
        let avid_state = self.avid_context.get_mut(&instance_id).unwrap();
        let indices = msg.indices();
        avid_state.fragments = Some(msg);
        
        // Start echo
        for index_msg in indices{
            let recipient = index_msg.recipient;
            let protocol_msg = ProtMsg::Echo(index_msg, instance_id);
            let sec_key = self.sec_key_map.get(&recipient).unwrap().clone();
            let wrapper_msg = WrapperMsg::new(protocol_msg.clone(), self.myid, &sec_key.as_slice());
            let cancel_handler: CancelHandler<Acknowledgement> = self.net_send.send(recipient, wrapper_msg).await;
            self.add_cancel_handler(cancel_handler);
        }        
    }
}

pub fn construct_merkle_tree(shards:Vec<Vec<u8>>, hc: &HashState)->MerkleTree{
    let hashes_rbc: Vec<Hash> = shards
        .into_iter()
        .map(|x| do_hash(x.as_slice()))
        .collect();

    MerkleTree::new(hashes_rbc, hc)
}