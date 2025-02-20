use std::collections::{HashMap, HashSet};

use crypto::{hash::Hash};
use types::Replica;

use crate::msg::{AVIDMsg, AVIDShard};

pub struct AVIDState{
    pub sender: usize,
    
    pub fragments: Option<AVIDMsg>,
    // Only for the recipient
    // deliveries tracked by the root Hash value
    
    pub deliveries: HashMap<Hash,HashMap<Replica,AVIDShard>>,
    pub message: Option<Vec<u8>>,

    pub echos: HashMap<Hash, HashSet<usize>>,
    // root Hash followed by all other composing hashes
    pub echo_roots: Option<(Hash,HashSet<Hash>)>,

    pub readys: HashMap<Hash, HashSet<usize>>,

    pub terminated: bool
}

impl AVIDState{
    
    pub fn new(sender: Replica)-> AVIDState{
        AVIDState {
            sender: sender,

            fragments: None, 
            message: None,
            deliveries: HashMap::default(),

            echos: HashMap::default(), 
            echo_roots: None, 
            
            readys: HashMap::default(), 
            
            terminated:false
        }
    }
}