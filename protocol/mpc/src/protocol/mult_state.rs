use std::{collections::{HashMap, HashSet}};

use crypto::hash::Hash;
use protocol::LargeField;
use types::Replica;

pub struct MultState{
    pub depth_share_map: HashMap<usize, SingleDepthState>
}

pub struct SingleDepthState{
    // Each party sends one share from each group. This map is sorted group wise
    pub l1_shares: Vec<(Vec<LargeField>,Vec<LargeField>)>,
    pub l1_shares_reconstructed: Vec<LargeField>,
    pub l2_shares: Vec<(Vec<LargeField>,Vec<LargeField>)>,
    pub l2_shares_reconstructed: Vec<LargeField>,

    pub util_rand_sharings: Vec<LargeField>,
    
    pub two_levels: bool,
    // TODO: replace these with mutexes
    pub recv_share_count_l1: usize,
    pub recv_share_count_l2: usize,

    pub recv_hash_set: HashSet<Hash>,
    pub recv_hash_msgs: Vec<Replica>
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
            recv_share_count_l1: 0,
            recv_share_count_l2: 0,

            recv_hash_set: HashSet::new(),
            recv_hash_msgs: Vec::new()
        }
    }
}

impl MultState{
    pub fn new() -> Self {
        MultState{
            depth_share_map: HashMap::new()
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