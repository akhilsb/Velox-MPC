use std::collections::HashMap;

use protocol::LargeField;

pub struct VerificationState{
    // A vector of multiplication tuples (a,b,a*b) to be verified at each depth
    pub mult_tuples: HashMap<usize, (Vec<LargeField>, Vec<LargeField>, Vec<LargeField>)>,
    pub compression_levels_shares: HashMap<usize, (Vec<LargeField>, Vec<LargeField>, Vec<LargeField>)>
}

impl VerificationState{
    pub fn new() -> Self {
        VerificationState{
            mult_tuples: HashMap::new(),
            compression_levels_shares: HashMap::new(),
        }
    }

    // Function to add a multiplication tuple for verification
    pub fn add_mult_inputs(&mut self, depth: usize, a_shares: Vec<LargeField>, b_shares: Vec<LargeField>,) {
        let entry = self.mult_tuples.entry(depth).or_insert_with(|| (Vec::new(), Vec::new(), Vec::new()));
        entry.0.extend(a_shares); // Add the shares of 'a' to the first vector
        entry.1.extend(b_shares); // Add the shares of 'b' to the second vector
    }

    pub fn add_mult_output_shares(&mut self, depth: usize, output_shares: Vec<LargeField>) {
        // For each multiplication tuple at this depth, we will assign the output share
        let entry = self.mult_tuples.entry(depth).or_insert_with(|| (Vec::new(), Vec::new(), Vec::new()));
        entry.2.extend(output_shares); // Add the shares of the output to the third vector
    }
}