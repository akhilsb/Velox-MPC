use std::collections::{HashMap, HashSet};

use crypto::hash::Hash;
use protocol::LargeFieldSer;
use types::Replica;

pub struct ACSSABState{
    // Shares, Nonce, Blinding nonce share in each tuple
    pub shares: HashMap<Replica, (Vec<LargeFieldSer>, LargeFieldSer, LargeFieldSer)>,
    // Commitments to shares, commitments to blinding polynomial, and DZK polynomial
    pub commitments: HashMap<Replica, (Vec<Hash>, Vec<Hash>, Vec<[u8;32]>)>,
    // Reliable Agreement
    pub ra_outputs: HashSet<Replica>,
    // Verification status for each party
    pub verification_status: HashMap<Replica, bool>,
    pub acss_status: HashSet<Replica>
}

impl ACSSABState{
    pub fn new() -> Self{
        Self{
            shares: HashMap::default(),
            commitments: HashMap::default(),
            ra_outputs: HashSet::default(),
            verification_status: HashMap::default(),
            acss_status: HashSet::default()
        }
    }
}