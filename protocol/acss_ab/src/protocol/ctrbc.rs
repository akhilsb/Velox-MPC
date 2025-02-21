use crate::Context;

impl Context{
    pub async fn handle_ctrbc_termination(&mut self, inst_id: usize, sender_rep: usize, content: Vec<u8>){
        log::info!("Received CTRBC termination message from sender {} for instance ID {}",sender_rep,inst_id);

        // Deserialize message
        let _comm_dzk_vals: (Vec<[u8;32]>,Vec<[u8;32]>,Vec<[u8;32]>) = bincode::deserialize(content.as_slice()).unwrap();
        log::info!("Deserialization successful for sender {} for instance ID {}",sender_rep,inst_id);
    }
}