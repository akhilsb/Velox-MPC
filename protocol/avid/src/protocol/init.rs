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
        // Each element of the vector is an AVID for sending a message to a single replica
        let mut avid_tree: Vec<(Replica,Vec<Vec<u8>>,MerkleTree)> = Vec::new(); 
        let mut roots_agg: Vec<u8> = Vec::new();
        
        for msg in msgs{
            // Get encrypted text itself
            let shards = get_shards(msg.1, self.num_faults+1, 2*self.num_faults);
            let merkle_tree = construct_merkle_tree(shards.clone(),&self.hash_context);
            roots_agg.extend(merkle_tree.root());
            avid_tree.push((msg.0,shards,merkle_tree));
        }
        
        let concise_root = do_hash(&roots_agg.as_slice());
        let sec_key_map = self.sec_key_map.clone();
        for (replica, sec_key) in sec_key_map.into_iter() {
            // TODO: Encryption
            let mut avid_shards = Vec::new();
            
            for avid_tree_elem in avid_tree.iter(){
                avid_shards.push(AVIDShard{
                    id: instance_id,
                    recipient: avid_tree_elem.0.clone(),
                    shard: avid_tree_elem.1.get(replica).unwrap().clone(),
                    proof: avid_tree_elem.2.gen_proof(replica),
                });
            }
            
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
        self.handle_echo(indices.clone(), self.myid, instance_id).await;
        let protocol_msg = ProtMsg::Echo(indices, instance_id);

        self.broadcast(protocol_msg).await;
    }
}

pub fn construct_merkle_tree(shards:Vec<Vec<u8>>, hc: &HashState)->MerkleTree{
    let hashes_rbc: Vec<Hash> = shards
        .into_iter()
        .map(|x| do_hash(x.as_slice()))
        .collect();

    MerkleTree::new(hashes_rbc, hc)
}