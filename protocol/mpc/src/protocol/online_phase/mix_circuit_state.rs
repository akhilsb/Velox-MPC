use std::collections::{HashMap, VecDeque};

use protocol::LargeField;

pub struct MixCircuitState{
    pub rand_bit_sharings: VecDeque<LargeField>,

    pub inputs: Vec<LargeField>,
    pub input_sharings: Vec<LargeField>,
    
    // log_^2(k) depths, k wires on each depth
    pub mult_result: HashMap<usize, Vec<LargeField>>,
    pub wire_sharings: HashMap<usize,Vec<LargeField>>,
    
    // k/2 pairs of wires on each depth
    pub wire_pairs: HashMap<usize, Vec<(LargeField, LargeField)>>,
    pub two_inverse: LargeField
}

impl MixCircuitState{
    pub fn new(masked_inputs: Vec<LargeField>) -> Self {
        MixCircuitState{
            rand_bit_sharings: VecDeque::new(),
            
            inputs: masked_inputs,
            input_sharings: Vec::new(),

            mult_result: HashMap::new(),
            wire_sharings: HashMap::new(),
            wire_pairs: HashMap::new(),

            two_inverse: LargeField::from(2 as u64).inv().unwrap()
        }
    }
}