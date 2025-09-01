# Scalable Anonymous Broadcast from  Asynchronous MPC using Velox

<img src="images/velox_logo.png" width="400"/>

This repository implements anonymous broadcast using Velox, an asynchronous MPC protocol. This code has been written as a research prototype and has not been vetted for security. Therefore, this repository can contain serious security vulnerabilities. Please use at your own risk. 

# Quick Start
We describe the steps to run this artifact. 

## Hardware and OS setup
1. This artifact has been run and tested on `x86_64` and `x64` architectures. However, we are unaware of any issues that would prevent this artifact from running on `x86` architectures. 

2. This artifact has been run and tested on Ubuntu OS (versions 20,22,24) following the Debian distro. However, we are unaware of any issues that would prevent this artifact from running on Fedora distros like CentOS and Red Hat Linux. 

## Rust installation and Cargo setup
The repository uses the `Cargo` build tool. The compatibility between dependencies has been tested for Rust version `1.83.0`.

3. **Install Rust and Cargo**: Run the set of following commands to install the toolchain required to compile code written in Rust and create binary executable files. 
```bash
sudo apt-get update
sudo apt-get -y upgrade
sudo apt-get -y autoremove
sudo apt-get -y install build-essential
sudo apt-get -y install cmake
sudo apt-get -y install curl
# Install rust (non-interactive)
curl --proto "=https" --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
source $HOME/.cargo/env
rustup install 1.83.0
rustup override set 1.83.0
```
4. Build the repository using the following command. The command should be run in the directory containing the `Cargo.toml` file. 
```bash
cargo build --release
mkdir logs
```
If the build fails because of lack of `lgmp` files, install the `libgmp3-dev` dependency using the following command and try again.
```
sudo apt-get install libgmp3-dev
```

5. **Generate Configuration Files**: Next, generate configuration files for nodes in the system using the following command. Run the following commands to create configuration files and necessary directories for logs storage. 
```bash
mkdir testdata/hyb_4
mkdir logs/
./target/release/config --base_port 15000 --client_base_port 19000 --client_run_port 19500 --NumNodes 4 --blocksize 100 --delay 100 --target testdata/hyb_4/ --local true
```

## Running the code
6. **Generate Inputs**: Generate files containing the inputs of each party. These files need to be placed in `testdata/inputs/` directory. A sample code in `python` has been provided to automatically generate these inputs. Navigate to the `testdata/inputs/` directory and run the following command. 
```bash
python3 gen_inputs.py
```
This command generates input text files of the form `input_{$i}.txt` in the `testdata/inputs/` folder. 

7. **Run the protocol**: After generating the configuration files, run the script `test.sh` in the scripts folder.
The protocol takes the following command line arguments.
- num_parties: The number of parties $n$ participating in the protocol. 
- num_messages: The anonymity set size `k`, which corresponds to the number of inputs to mix.  
- batchsize: ACSS parameter deciding number of secrets to be batched within each ACSS instance. 
- compression_factor: The degree of the polynomial in the multiplication tuple verification phase. A higher degree implies lower round complexity but higher computation complexity. 
```bash
./scripts/test.sh {num_parties} {num_messages} {batchsize} {compression_factor}
```
Substitute `{num_parties}` with the number of parties and `{num_messages}` with the `k` value, where `k` is the number of messages.  
Example values include `k=256,512,1024...`. 
An example run can be the following. 
```bash
./scripts/test.sh 4 256 1000 10
```
This script starts `n=4` parties. 
Each party $i$ reads the first `k/n` inputs from its input file `testdata/inputs/inputs_{$i}.txt`. 
Then, parties start the mixing protocol with `k` inputs. 

**Note: Each line in the input file must be less than 31 bytes. This is because the protocol converts the input into a finite field element. The code currently operates on a 254-bit finite field, so if the input is bigger, the encoding will fail.**

8. **Check results in logs**: The termination latencies of each protocol phase are logged into the `syncer-{}.log` file in logs directory. 
Please wait for a minute before checking the logfile.  
The output of individual parties can be found in individual log files `party-0-{}.log,...`. 
The `syncer-{}.log` file will contain phase-wise latencies of the protocol. 
As mentioned in the paper, the protocol contains four phases: (a) Preprocessing, (b) Online, (c) Verification, and (d) Output. 
The `syncer` module records the latency (in milliseconds) of each phase and will print it out to the log file in the following format. 
```
INFO [node::syncer] All n nodes completed the protocol for ID: 1 with latency [2961, 3241, 3457], status {"Preprocessing"}, and value {[]}
```
The array of latencies indicate the time at which each party terminated the protocol. 
In the output phase, the `syncer-{}.log` file will also contain the output of the protocol - a set of shuffled messages input to the protocol. 

9. **Kill processes**: Before running the protocol with another configuration, kill all processes running on the requested ports. 
```bash
sudo lsof -ti:15000-19500 | xargs kill -9
```

## Repository Structure

This repository implements scalable anonymous broadcast using asynchronous Multi-Party Computation (MPC) with the Velox protocol. Here's a high-level overview of the directory structure:

```
mpc/
├── protocol/           # Core MPC protocol implementations
│   ├── acss_ab/       # Asynchronous Complete Secret Sharing with Abort
│   ├── avid_ab/       # Asynchronous Verifiable Information Dispersal with Abort
│   ├── mpc/           # Main MPC protocols(multiplication, online phase, verification)
│   └── sh2t/          # Degree-2t sharing with Abort
│
├── node/              # Executable node implementation and coordination logic
├── benchmark/         # AWS benchmarking infrastructure and analysis tools
├── testdata/          # Configuration files and test inputs for different node setups
├── scripts/           # Execution scripts (test.sh for running protocols)
├── logs/              # Runtime logs from protocol execution
└── images/            # Project assets (logo, etc.)
```

## Running in AWS
Please refer to the `benchmark/` directory for instructions to run an AWS benchmark.

## Dependencies in the codebase
The artifact is organized into the following modules of code.

1. The config directory contains code pertaining to configuring each node in the distributed system. 
Each node requires information about port to use, network addresses of other nodes, symmetric keys to establish pairwise authenticated channels between nodes, and protocol specific configuration parameters. 
Code related to managing and parsing these parameters is in the config directory. 
This library has been borrowed from the libchatter (https://github.com/libdist-rs/libchatter-rs) repository.

2. Networking: This repository uses the libnet-rs (https://github.com/libdist-rs/libnet-rs) networking library. 
Similar libraries include networking library from the narwhal (https://github.com/MystenLabs/sui/tree/main/narwhal/) repository. The nodes use the tcp protocol to send messages to each other.

3. The protocol directory contains code that implements the building blocks of the codebase. 
The protocol employs ACSS, AVID, and Sh2t protocols, which build on smaller building blocks like Reliable Broadcast, Reliable Agreement, and Asynchronous consensus. 
These building blocks have been implemented in the Secure Distributed Computing repository (https://github.com/akhilsb/Secure-Distributed-Computing-Protocols). 

## Architecture
The following architecture diagram describes the components of Velox and their dependencies. 
The diagram can be interpreted as a Directed Graph with source vertices.
Each source vertex has been implemented using the composing building blocks from the Secure Distributed Computing Repository (https://github.com/akhilsb/Secure-Distributed-Computing-Protocols).

<img src="images/MPC_component_flow_updated.png"/>

The components Reliable Broadcast, Asynchronous Verifiable Information Dispersal, and Asynchronous Common Subset have been implemented in Secure Distributed Computing Protocols repository (https://github.com/akhilsb/Secure-Distributed-Computing-Protocols).
The remaining components have been implemented in this repository. 