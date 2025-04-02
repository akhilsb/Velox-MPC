use std::collections::HashMap;

use protocol::LargeField;

use super::ex_compr_state::ExComprState;

pub struct VerificationState{
    // A vector of multiplication tuples (a,b,a*b) to be verified at each depth
    pub mult_tuples: HashMap<usize, (Vec<LargeField>, Vec<LargeField>, Vec<LargeField>)>,
    pub ex_compr_state: HashMap<usize, ExComprState>,
}

impl VerificationState{
    pub fn new() -> Self {
        VerificationState{
            mult_tuples: HashMap::new(),
            ex_compr_state: HashMap::new(),
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

    pub fn add_compression_level_state(&mut self, 
        depth: usize, 
        x_shares: Vec<Vec<LargeField>>, 
        y_shares: Vec<Vec<LargeField>>, 
        z_shares: Vec<LargeField>
    ){
        let entry = self.ex_compr_state.entry(depth).or_insert_with(|| ExComprState::new(depth) );
        // Add the shares of x
        entry.x_sharings.extend(x_shares);
        // Add the shares of y
        entry.y_sharings.extend(y_shares);
        // Add the shares of z
        entry.mult_sharings.extend(z_shares);
    }
}