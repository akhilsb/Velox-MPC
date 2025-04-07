use std::{collections::{HashMap, HashSet}};

use crypto::hash::Hash;
use protocol::LargeField;
use types::Replica;

pub struct MultState{
    pub depth_share_map: HashMap<usize, SingleDepthState>,
    pub output_layer: OutputLayerState,
}

pub struct SingleDepthState{
    // Each party sends one share from each group. This map is sorted group wise
    pub l1_shares: Vec<(Vec<LargeField>,Vec<LargeField>)>,
    pub l1_shares_reconstructed: Vec<LargeField>,
    pub l2_shares: Vec<(Vec<LargeField>,Vec<LargeField>)>,
    pub l2_shares_reconstructed: Vec<LargeField>,

    pub util_rand_sharings: Vec<LargeField>,
    
    pub two_levels: bool,
    pub padding_shares: usize, 
    // TODO: replace these with mutexes
    pub recv_share_count_l1: usize,
    pub recv_share_count_l2: usize,

    pub recv_hash_set: HashSet<Hash>,
    pub recv_hash_msgs: Vec<Replica>,

    pub depth_terminated: bool,
}

impl SingleDepthState{
    pub fn new(two_levels: bool) -> Self {
        SingleDepthState{
            l1_shares: Vec::new(),
            l1_shares_reconstructed: Vec::new(),
            
            l2_shares: Vec::new(),
            l2_shares_reconstructed: Vec::new(),
            
            util_rand_sharings: Vec::new(),

            two_levels,
            padding_shares: 0,

            recv_share_count_l1: 0,
            recv_share_count_l2: 0,

            recv_hash_set: HashSet::new(),
            recv_hash_msgs: Vec::new(),

            depth_terminated: false,
        }
    }
}

pub struct OutputLayerState{
    pub output_wire_shares: HashMap<usize, (LargeField,Vec<LargeField>)>,
    pub reconstructed_masked_outputs: Option<Vec<LargeField>>,

    // CTRBC outputs
    pub broadcasted_masked_outputs: HashMap<Replica,Vec<u8>>,
    pub acs_output: Vec<Replica>,

    pub random_mask_shares: HashMap<usize, (LargeField,Vec<LargeField>)>,
}

impl OutputLayerState{
    pub fn new() -> Self {
        OutputLayerState{
            output_wire_shares: HashMap::default(),
            reconstructed_masked_outputs: None,

            broadcasted_masked_outputs: HashMap::default(),
            acs_output: Vec::new(),

            random_mask_shares: HashMap::default()
        }
    }
}

impl MultState{
    pub fn new() -> Self {
        MultState{
            depth_share_map: HashMap::new(),
            output_layer: OutputLayerState::new()   
        }
    }

    pub fn get_single_depth_state(&mut self, depth: usize, two_levels: bool, tot_groups_in_level: usize) -> &mut SingleDepthState {
        let mut single_depth_state = SingleDepthState::new(two_levels);

        // Fill vectors of this structure
        for _ in 0..tot_groups_in_level {
            // For each group, we will have a vector of pairs (x,y) for each party
            single_depth_state.l1_shares.push((Vec::new(), Vec::new()));
            single_depth_state.l2_shares.push((Vec::new(), Vec::new()));
        }

        self.depth_share_map.entry(depth).or_insert_with(|| single_depth_state)
    }
}