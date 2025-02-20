use crate::msg::AVIDIndexMsg;
use crate::{AVIDState};

use crate::{ProtMsg};

use crate::Context;
impl Context {
    // TODO: handle ready
    pub async fn handle_ready(self: &mut Context, indices: AVIDIndexMsg, ready_sender: usize, instance_id:usize){
        log::trace!("Received {:?} as ready", indices);
        
        if !self.avid_context.contains_key(&instance_id){
            let avid_state = AVIDState::new(indices.origin);
            self.avid_context.insert(instance_id, avid_state);
        }
        
        let avid_context = self.avid_context.get_mut(&instance_id).unwrap();

        if avid_context.terminated{
            // RBC Already terminated, skip processing this message
            return;
        }

        if !indices.verify_root(){

            log::error!("Concise root verification failed for echo sent by node {} initiated by node {}",ready_sender,indices.origin);
            return;
        }

        let ready_senders = avid_context.readys.entry(indices.concise_root).or_default();

        if ready_senders.contains(&ready_sender){
            return;
        }

        ready_senders.insert(ready_sender);

        let size = ready_senders.len().clone();

        if size == self.num_faults + 1{

            // Sent ECHOs and getting a ready message for the same ECHO
            if avid_context.echo_roots.is_some() && avid_context.echo_roots.clone().unwrap().0 == indices.concise_root{
                
                // If the echo_root variable is set, then we already sent ready for this message.
                // Nothing else to do here. Quit the execution. 

                return;
            }
            
            let avid_ready_msg = indices.clone();
            // Insert own ready
            avid_context.readys.get_mut(&indices.concise_root).unwrap().insert(self.myid);

            let ready_msg = ProtMsg::Ready(avid_ready_msg, instance_id);
            self.broadcast(ready_msg).await;
        }
        else if size == self.num_nodes - self.num_faults {
            log::info!("Received n-f READY messages for RBC Instance ID {}, terminating",instance_id);
            // Terminate protocol
            avid_context.terminated = true;
            // Send message to recipients and terminate
            let fragment = avid_context.fragments.clone();
            self.terminate(fragment,instance_id).await;
        }
    }
}
