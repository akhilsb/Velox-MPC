use std::collections::HashSet;

use crypto::aes_hash::{HashState, Proof};

use crypto::hash::{do_hash, Hash};
use serde::{Deserialize, Serialize};

use types::{Replica};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AVIDShard{
    pub id: usize,
    pub origin: Replica,
    pub recipient: Replica,
    pub shard: Vec<u8>,
    pub proof: Proof,
    pub master_proof: Proof,
}

impl AVIDShard{
    pub fn verify(&self, hash_state: &HashState)->bool{
        let hash_of_shard: [u8; 32] = do_hash(self.shard.as_slice());
        // log::info!("Hash of shard {:?}, 
        // Hash of shard from proof {:?}, root of proof {:?},
        // Hash of shard from master proof {:?}", 
        //     hash_of_shard, 
        //     self.proof.item(), 
        //     self.proof.root(),
        //     self.master_proof.item());
        return 
            (hash_of_shard == self.proof.item()) && 
            self.proof.validate(hash_state) && 
            (self.proof.root() == self.master_proof.item()) && 
            self.master_proof.validate(hash_state);
    }

    pub fn index_from_shard(&self)-> AVIDIndexMsg{
        AVIDIndexMsg { id: self.id, recipient: self.recipient, proof: self.master_proof.clone(), origin:  self.origin}
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AVIDMsg {
    // Batched AVID for disseminating messages to multiple parties
    // ID of dissemination, recipient, (Shard, Merkle Proof)
    pub shards: Vec<AVIDShard>,
    pub origin: Replica,
    pub concise_root: Hash
}

impl AVIDMsg {
    
    pub fn verify_mr_proofs(&self, hf: &HashState) -> bool {
        let mut state = true;
        // 2. Validate Merkle Proofs
        let mut hashes_vec: HashSet<Hash> = HashSet::default();

        for avid_state in self.shards.iter(){
            state = state&& avid_state.verify(hf);
            hashes_vec.insert(avid_state.master_proof.root());
        }

        state = state && hashes_vec.len() == 1;
        return state;
    }

    pub fn new(shards: Vec<AVIDShard>, origin: Replica)-> AVIDMsg{
        // create concise root
        let mut hash_vec : Vec<u8> = Vec::new();
        for shard in shards.iter(){
            hash_vec.extend(shard.proof.root());
        }
        let root_hash = do_hash(&hash_vec.as_slice());
        AVIDMsg { 
            shards: shards, 
            origin: origin, 
            concise_root: root_hash 
        }
    }

    pub fn indices(&self) -> Vec<AVIDIndexMsg>{
        
        let mut index_vec = Vec::new();
        for shard in &self.shards{
            index_vec.push(shard.index_from_shard());
        }
        index_vec
    }
    
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AVIDIndexMsg{
    pub id: usize,
    pub origin: Replica,
    pub recipient: Replica,
    pub proof: Proof,
}

impl AVIDIndexMsg{

    pub fn new(avidmsg: &AVIDMsg)-> Vec<AVIDIndexMsg>{
        
        let mut index_msgs = Vec::new();
        for shard in avidmsg.shards.iter(){
            index_msgs.push(AVIDIndexMsg{
                id: shard.id,
                recipient: shard.recipient,
                proof: shard.master_proof.clone(),
                origin: avidmsg.origin
            });
        }
        index_msgs
    }
}
/*
this is how the rbc protocol works
1. <sendall, m> (this is broadcast)
2. <echo, m>
3. on (2t+1 <echo, m>) <Ready, m>
4. on (t+1 <ready, m>) <ready, m>
5. on (2t+1 <ready, m>) output m, terminate
*/

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum ProtMsg {
    // Create your custom types of messages'
    Init(AVIDMsg, usize), // Init
    // ECHO contains only indices and roots. 
    Echo(AVIDIndexMsg,usize),
    // READY contains only indices and roots.
    Ready(Hash, Replica, Option<AVIDShard>,usize),
}