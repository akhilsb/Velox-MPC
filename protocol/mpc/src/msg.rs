use serde::{Serialize, Deserialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum ProtMsg{
    // Encrypted shares, and the depth of the circuit
    SharesL1(Vec<u8>, usize),
    SharesL2(Vec<u8>),
    ReconstructCoin()
}