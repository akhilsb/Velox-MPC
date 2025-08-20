# Scalable Asynchronous MPC from Lightweight Cryptography

<img src="images/velox_logo.png" width="400"/>

This repository implements anonymous broadcast using Velox, an asynchronous MPC protocol. This code has been written as a research prototype and has not been vetted for security. Therefore, this repository can contain serious security vulnerabilities. Please use at your own risk. 

# Quick Start
We describe the steps to run this artifact. 

## Hardware and OS setup
1. This artifact has been run and tested on `x86_64` and `x64` architectures. However, we are unaware of any issues that would prevent this artifact from running on `x86` architectures. 

2. This artifact has been run and tested on Ubuntu OS (versions 20,22,24) following the Debian distro. However, we are unaware of any issues that would prevent this artifact from running on Fedora distros like CentOS and Red Hat Linux. 

## Rust installation and Cargo setup
The repository uses the `Cargo` build tool. The compatibility between dependencies has been tested for Rust version `1.83.0`.

3. Run the set of following commands to install the toolchain required to compile code written in Rust and create binary executable files. 
```
$ sudo apt-get update
$ sudo apt-get -y upgrade
$ sudo apt-get -y autoremove
$ sudo apt-get -y install build-essential
$ sudo apt-get -y install cmake
$ sudo apt-get -y install curl
# Install rust (non-interactive)
$ curl --proto "=https" --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
$ source $HOME/.cargo/env
$ rustup install 1.83.0
$ rustup override set 1.83.0
```
4. Build the repository using the following command. The command should be run in the directory containing the `Cargo.toml` file. 
```
$ cargo build --release
$ mkdir logs
```
If the build fails because of lack of `lgmp` files, install the `libgmp3-dev` dependency using the following command and try again.
```
sudo apt-get install libgmp3-dev
```

5. Next, generate configuration files for nodes in the system using the following command. Make sure to create the directory (in this example, `testdata/hyb_4/`) before running this command. 
```
$ ./target/release/genconfig --base_port 15000 --client_base_port 19000 --client_run_port 19500 --NumNodes 4 --blocksize 100 --delay 100 --target testdata/hyb_4/ --local true
```

6. After generating the configuration files, run the script `test.sh` in the scripts folder with the following command line arguments. This command starts the protocol with `4` parties. 
```
$ ./scripts/test.sh testdata/hyb_4/syncer {num_messages} {batchsize} {compression_factor}
```

7. Substitute `{num_messages}` with the `k` value, where `k` is the number of messages.  Example values include `k=256,512,1024...`. The `{batchsize}` and `{compression_factor}` parameters tune the protocol for performance. Currently, the protocol only takes the number `k` as input. It generates a random set of `k` messages for broadcast. This can be improved by making the protocol instead take an input from a file. `{batchsize}` decides how many secrets the protocol packs into a single ACSS-Ab instance. `{compression_factor}` is the level of compression in the verification phase. Please refer to the paper for more details. 


8. The termination latencies of each protocol phase are logged into the `syncer.log` file in logs directory. The output of the protocol can be found in individual log files `0.log,...`. 

# Kill previous processes running on these ports
$ sudo lsof -ti:7000-7015,8500-8515,9000-9015,5000 | xargs kill -9
$ ./scripts/test.sh testdata/hyb_4/syncer 256 2000 10
```

## Running in AWS
TODO