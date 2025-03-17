use std::collections::{HashMap, HashSet};

use crypto::hash::Hash;
use protocol::LargeFieldSer;
use types::Replica;

pub struct Sh2tState{
    // Shares, Nonce, Blinding nonce share in each tuple
    pub shares: HashMap<Replica, (Vec<LargeFieldSer>, LargeFieldSer)>,
    // Commitments to shares, commitments to blinding polynomial, and DZK polynomial
    pub commitments: HashMap<Replica, Vec<Hash>>,
    // Reliable Agreement
    pub ra_outputs: HashSet<Replica>,
    // Verification status for each party
    pub verification_status: HashMap<Replica, bool>,
    pub status: HashSet<Replica>
}

impl Sh2tState{
    pub fn new() -> Self{
        Self{
            shares: HashMap::default(),
            commitments: HashMap::default(),
            ra_outputs: HashSet::default(),
            verification_status: HashMap::default(),
            status: HashSet::default()
        }
    }
}