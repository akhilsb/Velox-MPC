use std::{
    collections::HashMap,
    net::{SocketAddr, SocketAddrV4},
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::{anyhow, Result};
use config::Node;

use fnv::FnvHashMap;
use lambdaworks_math::{traits::ByteConversion};
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

use crate::protocol::ACSSABState;

pub struct Context {
    /// Data context
    pub num_nodes: usize,
    pub myid: usize,
    pub num_faults: usize,

    /// Secret Key map
    pub sec_key_map: HashMap<Replica, Vec<u8>>,

    /// Hardware acceleration context
    pub hash_context: HashState,

    /// Cancel Handlers
    pub cancel_handlers: HashMap<u64, Vec<CancelHandler<Acknowledgement>>>,
    exit_rx: oneshot::Receiver<()>,
    
    // Each Reliable Broadcast instance is associated with a Unique Identifier. 
    pub acss_ab_state: ACSSABState,

    // Maximum number of RBCs that can be initiated by a node. Keep this as an identifier for RBC service. 
    pub threshold: usize, 

    pub max_id: usize, 

    pub num_threads: usize,
    /// Input and output message queues for Reliable Broadcast
    pub inp_acss: Receiver<Vec<LargeFieldSer>>,
    pub out_acss: Sender<(Replica,Option<Vec<LargeFieldSer>>)>,

    /// CTRBC input and output channels
    pub inp_ctrbc: Sender<Vec<u8>>,
    pub recv_out_ctrbc: Receiver<(usize,usize, Vec<u8>)>,

    /// AVID input and output channels
    pub inp_avid_channel: Sender<Vec<(Replica,Option<Vec<u8>>)>>,
    pub recv_out_avid: Receiver<(Replica,Option<Vec<u8>>)>,

    /// RA input and output channels
    pub inp_ra_channel: Sender<(usize,usize,usize)>,
    pub recv_out_ra: Receiver<(usize,Replica,usize)>,

    pub use_fft: bool,
}

impl Context {
    pub fn spawn(
        config: Node,
        input_msgs: Receiver<Vec<LargeFieldSer>>, 
        output_msgs: Sender<(Replica,Option<Vec<LargeFieldSer>>)>, 
        _byz: bool
    ) -> anyhow::Result<oneshot::Sender<()>> {
        // Add a separate configuration for RBC service. 

        let mut ctrbc_config = config.clone();
        let mut avid_config = config.clone();
        let mut ra_config = config.clone();

        let port_rbc: u16 = 150;
        let port_avid: u16 = 300;
        let port_ra: u16 = 450;

        let mut consensus_addrs: FnvHashMap<Replica, SocketAddr> = FnvHashMap::default();
        for (replica, address) in config.net_map.iter() {
            let address: SocketAddr = address.parse().expect("Unable to parse address");

            let ctrbc_address: SocketAddr = SocketAddr::new(address.ip(), address.port() + port_rbc);
            let avid_address: SocketAddr = SocketAddr::new(address.ip(), address.port() + port_avid);
            let ra_address: SocketAddr = SocketAddr::new(address.ip(), address.port() + port_ra);

            ctrbc_config.net_map.insert(*replica, ctrbc_address.to_string());
            avid_config.net_map.insert(*replica, avid_address.to_string());
            ra_config.net_map.insert(*replica, ra_address.to_string());

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
        
        let (ra_req_send_channel, ra_req_recv_channel) = channel(10000);
        let (ra_out_send_channel, ra_out_recv_channel) = channel(10000);
        tokio::spawn(async move {
            let mut c = Context {
                num_nodes: config.num_nodes,
                sec_key_map: HashMap::default(),
                hash_context: hashstate,
                myid: config.id,
                
                num_faults: config.num_faults,
                cancel_handlers: HashMap::default(),
                exit_rx: exit_rx,
                
                acss_ab_state: ACSSABState::new(),
                threshold: 10000,

                max_id: rbc_start_id,
                
                num_threads: 4,
                inp_acss: input_msgs,
                out_acss: output_msgs,

                inp_ctrbc: ctrbc_req_send_channel,
                recv_out_ctrbc: ctrbc_out_recv_channel,

                inp_avid_channel: avid_req_send_channel,
                recv_out_avid: avid_out_recv_channel,

                inp_ra_channel: ra_req_send_channel,
                recv_out_ra: ra_out_recv_channel,

                use_fft: false
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

        let _status = ra::Context::spawn(
            ra_config,
            ra_req_recv_channel,
            ra_out_send_channel,
            false
        );

        let mut signals = Signals::new(&[SIGINT, SIGTERM])?;
        signals.forever().next();
        log::error!("Received termination signal");
        Ok(exit_tx)
    }

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
                    log::info!("Received termination event from CTRBC channel from party {} at time: {:?}", ctrbc_msg.1, SystemTime::now()
                                .duration_since(UNIX_EPOCH)
                                .unwrap()
                                .as_millis());
                    self.handle_ctrbc_termination(ctrbc_msg.0,ctrbc_msg.1,ctrbc_msg.2).await;
                },
                avid_msg = self.recv_out_avid.recv() =>{
                    let avid_msg = avid_msg.ok_or_else(||
                        anyhow!("Networking layer has closed")
                    )?;
                    if avid_msg.1.is_none(){
                        log::error!("Received None from AVID for sender {}", avid_msg.0);
                        continue;
                    }
                    log::info!("Received termination event from AVID channel from party {} at time: {:?}", avid_msg.0, SystemTime::now()
                                .duration_since(UNIX_EPOCH)
                                .unwrap()
                                .as_millis());
                    
                    self.handle_avid_termination(avid_msg.0,avid_msg.1).await;
                },
                ra_msg = self.recv_out_ra.recv() => {
                    let ra_msg = ra_msg.ok_or_else(||
                        anyhow!("Networking layer has closed")
                    )?;
                    log::info!("Received termination event from RA channel from party {} messages at time: {:?}", ra_msg.1, SystemTime::now()
                                .duration_since(UNIX_EPOCH)
                                .unwrap()
                                .as_millis());
                    
                }
            };
        }
        Ok(())
    }
}

pub fn to_socket_address(ip_str: &str, port: u16) -> SocketAddr {
    let addr = SocketAddrV4::new(ip_str.parse().unwrap(), port);
    addr.into()
}
