use crypto::aes_hash::{HashState, Proof};

use crypto::hash::{do_hash, Hash};
use serde::{Deserialize, Serialize};

use types::{Replica};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AVIDShard{
    
    pub id: usize,
    pub recipient: Replica,
    pub shard: Vec<u8>,
    pub proof: Proof

}

impl AVIDShard{
    pub fn verify(&self, hash_state: &HashState)->bool{
        let hash_of_shard: [u8; 32] = do_hash(self.shard.as_slice());
        return hash_of_shard == self.proof.item().clone() && self.proof.validate(hash_state);
    }

    pub fn index_from_shard(&self)-> AVIDIndex{
        AVIDIndex { id: self.id, recipient: self.recipient, root: self.proof.root() }
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
        let mut hashes_vec: Vec<u8> = Vec::new();

        for avid_state in self.shards.iter(){
            state = state&& avid_state.verify(hf);
            hashes_vec.extend(avid_state.proof.root());
        }

        state = state && (do_hash(&hashes_vec.as_slice()) == self.concise_root);
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

    pub fn indices(&self) -> AVIDIndexMsg{
        
        let mut index_vec = Vec::new();
        for shard in &self.shards{
            index_vec.push(shard.index_from_shard());
        }
        AVIDIndexMsg { 
            shard_indices: index_vec, 
            origin: self.origin, 
            concise_root: self.concise_root 
        }
    }
    
}


#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AVIDIndex{

    pub id: usize,
    pub recipient: Replica,
    pub root: Hash

}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AVIDIndexMsg{

    pub shard_indices : Vec<AVIDIndex>,
    pub origin: Replica,
    pub concise_root: Hash

}

impl AVIDIndexMsg{

    pub fn new(avidmsg: &AVIDMsg)-> AVIDIndexMsg{
        
        let mut shard_indices = Vec::new();
        for shard in avidmsg.shards.iter(){
            shard_indices.push(AVIDIndex{
                id: shard.id,
                recipient: shard.recipient,
                root: shard.proof.root()
            });
        }
        AVIDIndexMsg { 
            shard_indices: shard_indices, 
            origin: avidmsg.origin, 
            concise_root: avidmsg.concise_root 
        }

    }

    pub fn verify_root(&self) -> bool{

        let mut hash_vec = Vec::new();
        for index in &self.shard_indices{
            hash_vec.extend(index.root.clone());
        }

        return self.concise_root == do_hash(&hash_vec.as_slice());
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
    Ready(AVIDIndexMsg,usize),
    Deliver(AVIDShard,Replica,usize)
}