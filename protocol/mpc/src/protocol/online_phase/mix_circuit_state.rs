use std::collections::{HashMap, VecDeque};

use protocol::LargeField;
use types::Replica;

pub struct MixCircuitState{
    pub rand_bit_inp_shares: Vec<LargeField>,
    pub rand_bit_recon_shares: HashMap<usize, Vec<LargeField>>,
    
    pub rand_bit_inverse_recon_values: Vec<LargeField>,
    pub rand_bit_sharings: VecDeque<LargeField>,
    pub rand_bit_reconstruction: HashMap<usize, Vec<LargeField>>,

    pub input_acss_shares: HashMap<Replica, HashMap<usize,Vec<LargeField>>>,
    pub input_sharings: Vec<LargeField>,
    
    // log_^2(k) depths, k wires on each depth
    pub mult_result: HashMap<usize, Vec<LargeField>>,
    pub wire_sharings: HashMap<usize,Vec<LargeField>>,
    
    // k/2 pairs of wires on each depth
    pub wire_pairs: HashMap<usize, Vec<(LargeField, LargeField)>>,
    pub two_inverse: LargeField
}

impl MixCircuitState{
    pub fn new() -> Self {
        MixCircuitState{
            rand_bit_inp_shares: Vec::new(),
            rand_bit_recon_shares: HashMap::new(),

            rand_bit_inverse_recon_values: Vec::new(),
            rand_bit_sharings: VecDeque::new(),
            rand_bit_reconstruction: HashMap::default(),
            
            input_acss_shares: HashMap::default(),
            input_sharings: Vec::new(),

            mult_result: HashMap::new(),
            wire_sharings: HashMap::new(),
            wire_pairs: HashMap::new(),

            two_inverse: LargeField::from(2 as u64).inv().unwrap()
        }
    }
}