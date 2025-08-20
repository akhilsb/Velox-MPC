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

5. **Generate Configuration Files**: Next, generate configuration files for nodes in the system using the following command. Make sure to create the directory (in this example, `testdata/hyb_4/`) before running this command. 
```bash
./target/release/genconfig --base_port 15000 --client_base_port 19000 --client_run_port 19500 --NumNodes 4 --blocksize 100 --delay 100 --target testdata/hyb_4/ --local true
```

## Running the code
6. **Generate Inputs**: Generate files containing the inputs of each party. These files need to be placed in `testdata/inputs/` directory. A sample code in `python` has been provided to automatically generate these inputs. Navigate to the `testdata/inputs/` directory and run the following command. 
```bash
python3 gen_inputs.py
```
This command generates input text files of the form `input_{$i}.txt` in the `testdata/inputs/` folder. 

7. **Run the protocol**: After generating the configuration files, run the script `test.sh` in the scripts folder.
The protocol takes the following command line arguments.
- num_messages: The anonymity set size `k`, which corresponds to the number of inputs to mix.  
- batchsize: ACSS parameter deciding number of secrets to be batched within each ACSS instance. 
- compression_factor: The degree of the polynomial in the multiplication tuple verification phase. A higher degree implies lower round complexity but higher computation complexity. 
```bash
./scripts/test.sh {num_messages} {batchsize} {compression_factor}
```
Substitute `{num_messages}` with the `k` value, where `k` is the number of messages.  
Example values include `k=256,512,1024...`. 
An example run can be the following. 
```bash
./scripts/test.sh 256 1000 10
```
This script starts `n=4` parties. 
Each party $i$ reads the first `k/n` inputs from its input file `testdata/inputs/inputs_{$i}.txt`. 
Then, parties start the mixing protocol with `k` inputs. 

**Note: Each line in the input file must be less than 31 bytes. This is because the protocol converts the input into a finite field element. The code currently operates on a 254-bit finite field, so if the input is bigger, the encoding will fail.**

8. The termination latencies of each protocol phase are logged into the `syncer.log` file in logs directory. The output of the protocol can be found in individual log files `0.log,...`. 

9. Kill all processes running on the requested ports. 
```bash
sudo lsof -ti:15000-19500 | xargs kill -9
```

## Running in AWS
Please refer to the `benchmark/` directory for instructions to run an AWS benchmark. 