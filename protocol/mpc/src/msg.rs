use crypto::hash::Hash;
use protocol::LargeFieldSer;
use serde::{Serialize, Deserialize};
use types::Replica;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum ProtMsg{
    // Encrypted shares, and the depth of the circuit
    SharesL1(Vec<u8>, usize),
    SharesL2(Vec<u8>, usize),

    QuadShares(Vec<u8>, usize),
    // Hash Message to ensure at least t+1 parties are consistent with the hash value
    // Bool is for indicating linear or quadratic layer
    HashZMsg(Hash, usize, bool),
    ReconstructCoin(LargeFieldSer, usize),

    ReconstructVerfOutputSharing(LargeFieldSer, LargeFieldSer, LargeFieldSer),
    ReconstructMaskedOutput(Vec<LargeFieldSer>),

    ReconstructOutputMasks(Replica, Vec<LargeFieldSer>, LargeFieldSer, LargeFieldSer),

    // Temporary for testing
    ReconstructMultSharings(Vec<LargeFieldSer>, usize),
}