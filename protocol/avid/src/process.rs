use std::sync::Arc;

use super::{ProtMsg};
use crate::{context::Context, msg::AVIDMsg};
use crypto::hash::verf_mac;
use network::{plaintcp::CancelHandler, Acknowledgement};
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
                ProtMsg::Echo(main_msg, instance_id) => {
                    // RBC initialized
                    log::debug!("Received Echo for instance id {} from node : {}", instance_id, main_msg.origin);
                    self.handle_echo(main_msg, wrapper_msg.sender,instance_id).await;
                }
                ProtMsg::Ready(main_msg, instance_id) => {
                    // RBC initialized
                    log::debug!("Received Ready for instance id {} from node : {}", instance_id, main_msg.origin);
                    self.handle_ready(main_msg, wrapper_msg.sender,instance_id).await;
                }
                ProtMsg::Init(main_msg, instance_id) => {
                    // RBC initialized
                    log::debug!("Received Init for instance id {} from node : {}", instance_id, main_msg.origin);
                    self.handle_init(main_msg,instance_id).await;
                },
                ProtMsg::Deliver(avid_shard, origin, instance_id) => {
                    
                    log::debug!("Received Deliver for instance id {} from node : {}", instance_id, origin);
                    self.handle_deliver(avid_shard, origin, wrapper_msg.sender, instance_id).await;
                }
            }
        } else {
            log::warn!(
                "MAC Verification failed for message {:?}",
                wrapper_msg.protmsg
            );
        }
    }

    // Invoke this function once you terminate the protocol
    pub async fn terminate(&mut self, avid_opt:Option<AVIDMsg>, instance_id: usize) {
        //let fragment = avid_context.fragments.clone();
        //let avid_msg = fragment.unwrap();
        if avid_opt.is_some(){
            let avid_msg= avid_opt.unwrap();
            for avid_shard in avid_msg.shards{    
                let recipient = avid_shard.recipient.clone();
                if recipient == self.myid{
                    self.handle_deliver(avid_shard, avid_msg.origin, self.myid, instance_id).await;
                }
                else{
    
                    let sec_key = self.sec_key_map.get(&recipient).unwrap().clone();
                    let protocol_msg = ProtMsg::Deliver(avid_shard, avid_msg.origin, instance_id);
                    let wrapper_msg = WrapperMsg::new(protocol_msg.clone(),self.myid,&sec_key);
                    let cancel_handler: CancelHandler<Acknowledgement> = self.net_send.send(recipient, wrapper_msg).await;
                    self.add_cancel_handler(cancel_handler);
                
                }
            }
        }
    }
}
