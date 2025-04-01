use protocol::LargeFieldSer;
use serde::{Serialize, Deserialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum ProtMsg{
    // Encrypted shares, and the depth of the circuit
    SharesL1(Vec<u8>, usize),
    SharesL2(Vec<u8>, usize),

    QuadShares(Vec<u8>, usize),
    // Serialized share, depth of the circuit where the coin is being called
    ReconstructCoin(LargeFieldSer, usize)
}