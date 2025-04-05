use std::collections::{HashMap, HashSet};

use crypto::hash::Hash;
use lambdaworks_math::polynomial::Polynomial;
use protocol::{LargeField, AvssShare};
use types::Replica;

#[derive(Clone, Debug)]
pub struct ACSSABState{
    // Shares, Nonce, Blinding nonce share in each tuple
    pub shares: HashMap<Replica, AvssShare>,
    // Commitments to shares, commitments to blinding polynomial, and DZK polynomial
    pub commitments: HashMap<Replica, (Vec<Hash>, Vec<Hash>, Vec<[u8;32]>)>,
    // Reliable Agreement
    pub ra_outputs: HashSet<Replica>,
    // Verification status for each party
    pub verification_status: HashMap<Replica, bool>,
    pub acss_status: HashSet<Replica>,

    pub dzk_poly: HashMap<Replica,Polynomial<LargeField>>,
    pub commitment_root_fe: HashMap<Replica, LargeField>,
}

impl ACSSABState{
    pub fn new() -> Self{
        Self{
            shares: HashMap::default(),
            commitments: HashMap::default(),
            ra_outputs: HashSet::default(),
            verification_status: HashMap::default(),
            acss_status: HashSet::default(),

            dzk_poly: HashMap::default(),
            commitment_root_fe: HashMap::default(),
        }
    }
}