use std::{
    collections::HashMap,
    net::{SocketAddr, SocketAddrV4},
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::{anyhow, Result};
use config::Node;

use fnv::FnvHashMap;
use lambdaworks_math::{unsigned_integer::element::UnsignedInteger, traits::ByteConversion};
use network::{
    plaintcp::{CancelHandler},
    Acknowledgement,
};
use protocol::{LargeFieldSer, LargeField};
use signal_hook::{iterator::Signals, consts::{SIGINT, SIGTERM}};
use tokio::{sync::{
    mpsc::{Receiver, Sender, channel},
    oneshot,
}};
// use tokio_util::time::DelayQueue;
use types::{Replica};

use crypto::aes_hash::HashState;

pub struct Context {
    /// Data context
    pub num_nodes: usize,
    pub myid: usize,
    pub num_faults: usize,
    byz: bool,

    /// Secret Key map
    pub sec_key_map: HashMap<Replica, Vec<u8>>,

    /// Hardware acceleration context
    pub hash_context: HashState,

    /// Cancel Handlers
    pub cancel_handlers: HashMap<u64, Vec<CancelHandler<Acknowledgement>>>,
    exit_rx: oneshot::Receiver<()>,
    
    // Each Reliable Broadcast instance is associated with a Unique Identifier. 
    pub avid_context: HashMap<usize, usize>,

    // Maximum number of RBCs that can be initiated by a node. Keep this as an identifier for RBC service. 
    pub threshold: usize, 

    pub max_id: usize, 

    pub num_threads: usize,
    /// Input and output message queues for Reliable Broadcast
    pub inp_acss: Receiver<Vec<LargeFieldSer>>,
    pub out_acss: Sender<Vec<(Replica,Option<Vec<LargeFieldSer>>)>>,

    /// CTRBC input and output channels
    pub inp_ctrbc: Sender<Vec<u8>>,
    pub recv_out_ctrbc: Receiver<(usize,usize, Vec<u8>)>,

    /// AVID input and output channels
    pub inp_avid_channel: Sender<Vec<(Replica,Option<Vec<u8>>)>>,
    pub recv_out_avid: Receiver<(Replica,Option<Vec<u8>>)>
}

impl Context {
    pub fn spawn(
        config: Node,
        input_msgs: Receiver<Vec<LargeFieldSer>>, 
        output_msgs: Sender<Vec<(Replica,Option<Vec<LargeFieldSer>>)>>, 
        byz: bool
    ) -> anyhow::Result<oneshot::Sender<()>> {
        // Add a separate configuration for RBC service. 

        let mut ctrbc_config = config.clone();
        let mut avid_config = config.clone();

        let port_rbc: u16 = 150;
        let port_avid: u16 = 300;

        let mut consensus_addrs: FnvHashMap<Replica, SocketAddr> = FnvHashMap::default();
        for (replica, address) in config.net_map.iter() {
            let address: SocketAddr = address.parse().expect("Unable to parse address");

            let ctrbc_address: SocketAddr = SocketAddr::new(address.ip(), address.port() + port_rbc);
            let avid_address: SocketAddr = SocketAddr::new(address.ip(), address.port() + port_avid);

            ctrbc_config.net_map.insert(*replica, ctrbc_address.to_string());
            avid_config.net_map.insert(*replica, avid_address.to_string());

            consensus_addrs.insert(*replica, SocketAddr::from(address.clone()));

        }
        let (exit_tx, exit_rx) = oneshot::channel();

        // Keyed AES ciphers
        let key0 = [5u8; 16];
        let key1 = [29u8; 16];
        let key2 = [23u8; 16];
        let hashstate = HashState::new(key0, key1, key2);

        let threshold:usize = 10000;
        let rbc_start_id = threshold*config.id;

        let (ctrbc_req_send_channel, ctrbc_req_recv_channel) = channel(10000);
        let (ctrbc_out_send_channel, ctrbc_out_recv_channel) = channel(10000);

        let (avid_req_send_channel, avid_req_recv_channel) = channel(10000);
        let (avid_out_send_channel, avid_out_recv_channel) = channel(10000);
        tokio::spawn(async move {
            let mut c = Context {
                num_nodes: config.num_nodes,
                sec_key_map: HashMap::default(),
                hash_context: hashstate,
                myid: config.id,
                byz: byz,
                num_faults: config.num_faults,
                cancel_handlers: HashMap::default(),
                exit_rx: exit_rx,
                
                avid_context:HashMap::default(),
                threshold: 10000,

                max_id: rbc_start_id,
                
                num_threads: 4,
                inp_acss: input_msgs,
                out_acss: output_msgs,

                inp_ctrbc: ctrbc_req_send_channel,
                recv_out_ctrbc: ctrbc_out_recv_channel,

                inp_avid_channel: avid_req_send_channel,
                recv_out_avid: avid_out_recv_channel
            };

            // Populate secret keys from config
            for (id, sk_data) in config.sk_map.clone() {
                c.sec_key_map.insert(id, sk_data.clone());
            }

            // Run the consensus context
            if let Err(e) = c.run().await {
                log::error!("Consensus error: {}", e);
            }
        });

        let _status =  ctrbc::Context::spawn(
            ctrbc_config, 
            ctrbc_req_recv_channel, 
            ctrbc_out_send_channel, 
            false
        );

        let _status =  avid::Context::spawn(
            avid_config, 
            avid_req_recv_channel, 
            avid_out_send_channel, 
            false
        );

        let mut signals = Signals::new(&[SIGINT, SIGTERM])?;
        signals.forever().next();
        log::error!("Received termination signal");
        Ok(exit_tx)
    }

    // pub async fn broadcast(&mut self, protmsg: ProtMsg) {
    //     let sec_key_map = self.sec_key_map.clone();
    //     for (replica, sec_key) in sec_key_map.into_iter() {
    //         if self.byz && replica % 2 == 0 {
    //             // Simulates a crash fault
    //             continue;
    //         }
    //         if replica != self.myid {
    //             let wrapper_msg = WrapperMsg::new(protmsg.clone(), self.myid, &sec_key.as_slice());
    //             let cancel_handler: CancelHandler<Acknowledgement> =
    //                 self.net_send.send(replica, wrapper_msg).await;
    //             self.add_cancel_handler(cancel_handler);
    //         }
    //     }
    // }

    // pub fn add_cancel_handler(&mut self, canc: CancelHandler<Acknowledgement>) {
    //     self.cancel_handlers.entry(0).or_default().push(canc);
    // }

    // pub async fn send(&mut self, replica: Replica, wrapper_msg: WrapperMsg<ProtMsg>) {
    //     let cancel_handler: CancelHandler<Acknowledgement> =
    //         self.net_send.send(replica, wrapper_msg).await;
    //     self.add_cancel_handler(cancel_handler);
    // }

    pub async fn run(&mut self) -> Result<()> {
        // The process starts listening to messages in this process.
        // First, the node sends an alive message
        loop {
            tokio::select! {
                // Receive exit handlers
                exit_val = &mut self.exit_rx => {
                    exit_val.map_err(anyhow::Error::new)?;
                    log::info!("Termination signal received by the server. Exiting.");
                    break
                },
                acss_msg = self.inp_acss.recv() =>{
                    let secrets = acss_msg.ok_or_else(||
                        anyhow!("Networking layer has closed")
                    )?;
                    log::info!("Received request to start ACSS with abort  for {} secrets at time: {:?}",secrets.len() , SystemTime::now()
                                .duration_since(UNIX_EPOCH)
                                .unwrap()
                                .as_millis());
                    
                    let secrets_field: Vec<LargeField> = secrets.into_iter().map(|secret| LargeField::from_bytes_be(&secret).unwrap()).collect();
                    self.init_acss_ab(secrets_field).await;
                },
                ctrbc_msg = self.recv_out_ctrbc.recv() =>{
                    let ctrbc_msg = ctrbc_msg.ok_or_else(||
                        anyhow!("Networking layer has closed")
                    )?;
                    log::info!("Received request to start RBC from party {} messages at time: {:?}", ctrbc_msg.1, SystemTime::now()
                                .duration_since(UNIX_EPOCH)
                                .unwrap()
                                .as_millis());
                    self.handle_ctrbc_termination(ctrbc_msg.0,ctrbc_msg.1,ctrbc_msg.2).await;
                },
            };
        }
        Ok(())
    }
}

pub fn to_socket_address(ip_str: &str, port: u16) -> SocketAddr {
    let addr = SocketAddrV4::new(ip_str.parse().unwrap(), port);
    addr.into()
}
