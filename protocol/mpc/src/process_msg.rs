use std::sync::Arc;

use crate::{context::Context, msg::ProtMsg};
use crypto::hash::verf_mac;
use types::{WrapperMsg};

impl Context {
    // This function verifies the Message Authentication Code (MAC) of a sent message
    // A node cannot impersonate as another node because of MACs
    pub fn check_proposal(&self, wrapper_msg: Arc<WrapperMsg<ProtMsg>>) -> bool {
        // validate MAC
        let byte_val =
            bincode::serialize(&wrapper_msg.protmsg).expect("Failed to serialize object");
        let sec_key = match self.sec_key_map.get(&wrapper_msg.clone().sender) {
            Some(val) => val,
            None => {
                panic!("Secret key not available, this shouldn't happen")
            }
        };
        if !verf_mac(&byte_val, &sec_key.as_slice(), &wrapper_msg.mac) {
            log::warn!("MAC Verification failed.");
            return false;
        }
        true
    }

    pub(crate) async fn process_msg(&mut self, wrapper_msg: WrapperMsg<ProtMsg>) {
        log::trace!("Received protocol msg: {:?}", wrapper_msg);
        let msg = Arc::new(wrapper_msg.clone());

        // Verify the message's authenticity before proceeding
        if self.check_proposal(msg) {
            match wrapper_msg.clone().protmsg {
                ProtMsg::SharesL1(main_msg, depth) => {
                    // RBC initialized
                    log::debug!("Received L1 share message for depth {} from node : {}", depth, wrapper_msg.sender);
                    self.handle_l1_message(main_msg, depth, wrapper_msg.sender).await;
                }
                ProtMsg::SharesL2(main_msg, depth) => {
                    // RBC initialized
                    log::debug!("Received L2 share message for depth {} from node : {}", depth, wrapper_msg.sender);
                    self.handle_l2_message(main_msg, depth,wrapper_msg.sender).await;
                }
                ProtMsg::QuadShares(main_msg, depth) => {
                    // RBC initialized
                    log::debug!("Received Init for instance id {} from node : {}", depth, wrapper_msg.sender);
                    self.handle_quadratic_mult_shares(depth,main_msg, wrapper_msg.sender).await;
                },
                ProtMsg::HashZMsg(hash_val, depth, lin_or_quad) => {
                    // RBC initialized
                    log::debug!("Received HashZMsg for depth {} from node : {}", depth, wrapper_msg.sender);
                    self.handle_hash_broadcast(hash_val, depth, lin_or_quad, wrapper_msg.sender).await;
                },
                ProtMsg::ReconstructCoin(ser_share, depth) => {
                    log::debug!("Received ReconstructCoin message");
                    self.handle_common_coin_msg(ser_share, wrapper_msg.sender, depth).await;
                }
            }
        } else {
            log::warn!(
                "MAC Verification failed for message {:?}",
                wrapper_msg.protmsg
            );
        }
    }
}
