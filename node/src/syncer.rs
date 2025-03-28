use std::{collections::{HashSet, HashMap}, net::{SocketAddr,SocketAddrV4}, time::{SystemTime, UNIX_EPOCH, Duration}};

use anyhow::{Result, anyhow};
use mpc::handlers::SyncHandler;
use fnv::FnvHashMap;
use network::{plaintcp::{TcpReceiver, TcpReliableSender, CancelHandler}, Acknowledgement};
use tokio::{sync::{oneshot, mpsc::{unbounded_channel, UnboundedReceiver}}, time};
use types::{Replica, SyncMsg, SyncState, RBCSyncMsg};

//use std::fs::read_to_string;

pub struct Syncer{
    pub num_nodes: usize,
    pub ready_for_broadcast: bool,

    pub rbc_id:usize,
    pub rbc_msgs: HashMap<usize,String>,
    pub rbc_start_times: HashMap<usize,u128>,
    pub rbc_complete_times: HashMap<usize,HashMap<Replica,u128>>,
    pub rbc_comp_values: HashMap<usize,HashSet<String>>,

    //pub broadcast_msgs: Vec<String>,
    
    pub sharing_complete_times: HashMap<Replica,u128>,
    pub recon_start_time: u128,
    pub net_map: FnvHashMap<Replica,String>,
    pub alive: HashSet<Replica>,
    pub timings:HashMap<Replica,u128>,
    
    pub cli_addr: SocketAddr,
    
    pub rx_net: UnboundedReceiver<SyncMsg>,
    pub net_send: TcpReliableSender<Replica,SyncMsg,Acknowledgement>,
    
    exit_rx: oneshot::Receiver<()>,
    /// Cancel Handlers
    pub cancel_handlers: Vec<CancelHandler<Acknowledgement>>,
}

impl Syncer{
    pub fn spawn(
        net_map: FnvHashMap<Replica,String>,
        cli_addr:SocketAddr,
        //filename: String
    )-> anyhow::Result<oneshot::Sender<()>>{
        let (exit_tx, exit_rx) = oneshot::channel();
        let (tx_net_to_server, rx_net_to_server) = unbounded_channel();
        let cli_addr_sock = cli_addr.port();
        let new_sock_address = SocketAddrV4::new("0.0.0.0".parse().unwrap(), cli_addr_sock);
        TcpReceiver::<Acknowledgement, SyncMsg, _>::spawn(
            std::net::SocketAddr::V4(new_sock_address),
            SyncHandler::new(tx_net_to_server),
        );
        //let broadcast_msgs = read_lines(&filename);
        let mut server_addrs :FnvHashMap<Replica,SocketAddr>= FnvHashMap::default();
        for (replica,address) in net_map.iter(){
            let address:SocketAddr = address.parse().expect("Unable to parse address");
            server_addrs.insert(*replica, SocketAddr::from(address.clone()));
        }
        let net_send = TcpReliableSender::<Replica,SyncMsg,Acknowledgement>::with_peers(server_addrs);
        tokio::spawn(async move{
            let mut syncer = Syncer{
                net_map:net_map.clone(),
                ready_for_broadcast: false,

                rbc_id: 0,
                rbc_msgs: HashMap::default(),
                rbc_start_times: HashMap::default(),
                rbc_complete_times: HashMap::default(),
                rbc_comp_values:HashMap::default(),

                //broadcast_msgs: broadcast_msgs,

                sharing_complete_times:HashMap::default(),
                recon_start_time:0,
                num_nodes:net_map.len(),
                alive:HashSet::default(),
                
                timings:HashMap::default(),
                cli_addr:cli_addr,
                rx_net:rx_net_to_server,
                net_send:net_send,
                exit_rx:exit_rx,
                cancel_handlers:Vec::new()
            };
            if let Err(e) = syncer.run().await {
                log::error!("Consensus error: {}", e);
            }
        });
        Ok(exit_tx)
    }
    pub async fn broadcast(&mut self, sync_msg:SyncMsg){
        for replica in 0..self.num_nodes {
            let cancel_handler:CancelHandler<Acknowledgement> = self.net_send.send(replica, sync_msg.clone()).await;
            self.add_cancel_handler(cancel_handler);    
        }
    }
    pub async fn run(&mut self)-> Result<()>{
        let mut interval = time::interval(Duration::from_millis(100));
        loop {
            tokio::select! {
                // Receive exit handlers
                exit_val = &mut self.exit_rx => {
                    exit_val.map_err(anyhow::Error::new)?;
                    log::info!("Termination signal received by the server. Exiting.");
                    break
                },
                msg = self.rx_net.recv() => {
                    // Received a protocol message
                    // Received a protocol message
                    log::trace!("Got a message from the server: {:?}", msg);
                    let msg = msg.ok_or_else(||
                        anyhow!("Networking layer has closed")
                    )?;
                    match msg.state{
                        SyncState::ALIVE=>{
                            log::info!("Got ALIVE message from node {}",msg.sender);
                            self.alive.insert(msg.sender);
                            if self.alive.len() == self.num_nodes{
                                self.ready_for_broadcast = true;
                            }
                        },
                        SyncState::STARTED=>{
                            log::debug!("Node {} started the protocol",msg.sender);
                        },
                        SyncState::COMPLETED=>{
                            log::info!("Got COMPLETED message from node {}",msg.sender);
                            
                            // deserialize message
                            let rbc_msg: RBCSyncMsg = bincode::deserialize(&msg.value).expect("Unable to deserialize message received from node");
                            
                            let latency_map = self.rbc_complete_times.entry(rbc_msg.id).or_default();
                            latency_map.insert(msg.sender, SystemTime::now()
                            .duration_since(UNIX_EPOCH)
                            .unwrap()
                            .as_millis());
                            
                            let value_set = self.rbc_comp_values.entry(rbc_msg.id).or_default();
                            value_set.insert(rbc_msg.msg.to_string());
                            if latency_map.len() == self.num_nodes{

                                let start_time = self.rbc_start_times.get(&rbc_msg.id).unwrap();
                                // All nodes terminated protocol
                                
                                let mut vec_times = Vec::new();
                                for (_rep,time) in latency_map.iter(){
                                    vec_times.push(time.clone()-start_time);
                                }
                                
                                vec_times.sort();
                                
                                if value_set.len() > 1{
                                    log::info!("Received multiple values from nodes, broadcast failed, rerun test {:?}",value_set);
                                }
                                else{
                                    log::info!("All n nodes completed the protocol for ID: {} with latency {:?} and value {:?}",rbc_msg.id,vec_times,value_set);
                                }
                                //self.broadcast(SyncMsg { sender: self.num_nodes, state: SyncState::STOP, value:"Terminate".to_string().into_bytes()}).await;
                            }
                        }
                        _=>{}
                    }
                },
                _ = interval.tick() => {
                    if self.ready_for_broadcast{
                        // Initiate new broadcast
                        if self.rbc_id >= 1{
                            continue;
                        }
                        self.rbc_id += 1;
                        let sync_rbc_msg = RBCSyncMsg{
                            id: self.rbc_id,
                            //msg: self.broadcast_msgs.get(&self.rbc_id-1).unwrap().to_string(),
                            msg: "Start".to_string()
                        };
                        let binaryfy_val = bincode::serialize(&sync_rbc_msg).expect("Failed to serialize client message");
                        // let cancel_handler:CancelHandler<Acknowledgement> = self.net_send.send(0, SyncMsg { 
                        //     sender: self.num_nodes, 
                        //     state: SyncState::START,
                        //     value:binaryfy_val
                        // }).await;
                        // self.add_cancel_handler(cancel_handler);
                        log::info!("Initiating MPC with dealer with ID: {} and value {:?}",0, sync_rbc_msg);
                        self.broadcast(SyncMsg { 
                            sender: self.num_nodes, 
                            state: SyncState::START,
                            value: binaryfy_val
                        }).await;
                        
                        //self.add_cancel_handler(cancel_handler);

                        let start_time = SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap()
                        .as_millis();
                        self.rbc_start_times.insert(self.rbc_id, start_time);
                    }
                }
            }
        }
        Ok(())
    }
    pub fn add_cancel_handler(&mut self, canc: CancelHandler<Acknowledgement>){
        self.cancel_handlers
            .push(canc);
    }
}