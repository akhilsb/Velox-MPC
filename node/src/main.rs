use anyhow::{anyhow, Result};
use clap::{load_yaml, App};
use config::Node;
use fnv::FnvHashMap;
use node::Syncer;
use signal_hook::{
    consts::{SIGINT, SIGTERM},
    iterator::Signals,
};
use std::{net::{SocketAddr, SocketAddrV4}};

#[tokio::main]
async fn main() -> Result<()> {
    log::error!("{}", std::env::current_dir().unwrap().display());
    let yaml = load_yaml!("cli.yml");
    let m = App::from_yaml(yaml).get_matches();
    //println!("{:?}",m);
    let conf_str = m
        .value_of("config")
        .expect("unable to convert config file into a string");
    let vss_type = m
        .value_of("protocol")
        .expect("Unable to detect protocol to run");
    let _input_value = m.value_of("input").expect("Unable to read input string");
    let syncer_file = m
        .value_of("syncer")
        .expect("Unable to parse syncer ip file");
    let mixing_batch_size = m
        .value_of("batches")
        .expect("Unable to parse number of batches")
        .parse::<usize>().unwrap();
    let per_batch = m
        .value_of("per")
        .expect("Unable to parse per batch")
        .parse::<usize>().unwrap();
    let compression_factor = m
        .value_of("comp")
        .expect("Unable to parse compression factor")
        .parse::<usize>().unwrap();
    // let broadcast_msgs_file = m
    //     .value_of("bfile")
    //     .expect("Unable to parse broadcast messages file");
    let byz_flag = m.value_of("byz").expect("Unable to parse Byzantine flag");
    let node_normal: bool = match byz_flag {
        "true" => true,
        "false" => false,
        _ => {
            panic!("Byz flag invalid value");
        }
    };
    let conf_file = std::path::Path::new(conf_str);
    let str = String::from(conf_str);
    let mut config = match conf_file
        .extension()
        .expect("Unable to get file extension")
        .to_str()
        .expect("Failed to convert the extension into ascii string")
    {
        "json" => Node::from_json(str),
        "dat" => Node::from_bin(str),
        "toml" => Node::from_toml(str),
        "yaml" => Node::from_yaml(str),
        _ => panic!("Invalid config file extension"),
    };

    simple_logger::SimpleLogger::new()
        .with_utc_timestamps()
        .init()
        .unwrap();
    log::set_max_level(log::LevelFilter::Info);
    config.validate().expect("The decoded config is not valid");
    if let Some(f) = m.value_of("ip") {
        let f_str = f.to_string();
        log::info!("Logging the file f {}", f_str);
        config.update_config(util::io::file_to_ips(f.to_string()));
    }
    let config = config;
    // Start the Reliable Broadcast protocol
    let exit_tx;
    match vss_type {
        // "acs" => {
        //     exit_tx = 
        //         acs::Context::spawn(config, 
        //             batches, 
        //             per_batch, 
        //             true,
        //             node_normal
        //         ).unwrap();
        // }
        "mpc" => {
            exit_tx =
                mpc::Context::spawn(
                    config, 
                     per_batch,
                    mixing_batch_size,
                    compression_factor,
                    node_normal
                ).unwrap();
        }
        // "sh2t" => {
        //     let (_req_sender,req_receiver) = channel(10000);
        //     let (out_sender,_out_receiver) = channel(10000);
        //     exit_tx =
        //         sh2t::Context::spawn(
        //             config, 
        //             req_receiver, 
        //             out_sender, 
        //             node_normal
        //         ).unwrap();
        // }
        "sync" => {
            let f_str = syncer_file.to_string();
            log::info!("Logging the file f {}", f_str);
            let ip_str = util::io::file_to_ips(f_str);
            let mut net_map = FnvHashMap::default();
            let mut idx = 0;
            for ip in ip_str {
                net_map.insert(idx, ip.clone());
                idx += 1;
            }
            //let client_addr = net_map.get(&(net_map.len()-1)).unwrap();
            //exit_tx = Syncer::spawn(net_map, config.client_addr.clone(),broadcast_msgs_file.to_string()).unwrap();
            exit_tx = Syncer::spawn(net_map, config.client_addr.clone()).unwrap();
        }
        _ => {
            log::error!(
                "Matching VSS not provided {}, canceling execution",
                vss_type
            );
            return Ok(());
        }
    }
    //let exit_tx = pedavss_cc::node::Context::spawn(config).unwrap();
    // Implement a waiting strategy
    let mut signals = Signals::new(&[SIGINT, SIGTERM])?;
    signals.forever().next();
    log::error!("Received termination signal");
    exit_tx
        .send(())
        .map_err(|_| anyhow!("Server already shut down"))?;
    log::error!("Shutting down server");
    Ok(())
}

pub fn to_socket_address(ip_str: &str, port: u16) -> SocketAddr {
    let addr = SocketAddrV4::new(ip_str.parse().unwrap(), port);
    addr.into()
}
