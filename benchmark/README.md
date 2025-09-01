# Running Benchmarks
Based on [Narwhal](https://github.com/facebookresearch/narwhal). 

This document explains how to benchmark the codebase and read benchmarks' results. It also provides a step-by-step tutorial to run benchmarks on [Amazon Web Services (AWS)](https://aws.amazon.com) across multiple data centers (WAN).

The configuration requirements for this protocol can be specified in the `fabfile.py` file in the first four lines of code. 
```python
n = 16
num_messages = 256
batch_size = 1000
compression_factor = 10
```

## Setup
The core protocols are written in Rust, but all benchmarking scripts are written in Python and run with [Fabric](http://www.fabfile.org/). To run the remote benchmark, install the python dependencies:

```
$ pip install -r requirements.txt
```

You also need to install [tmux](https://linuxize.com/post/getting-started-with-tmux/#installing-tmux) (which runs all nodes and clients in the background). 

## AWS Benchmarks
This repo integrates various python scripts to deploy and benchmark the codebase on [Amazon Web Services (AWS)](https://aws.amazon.com). They are particularly useful to run benchmarks in the WAN, across multiple data centers. This section provides a step-by-step tutorial explaining how to use them.

### Step 1. Set up your AWS credentials
Set up your AWS credentials to enable programmatic access to your account from your local machine. These credentials will authorize your machine to create, delete, and edit instances on your AWS account programmatically. First of all, [find your 'access key id' and 'secret access key'](https://docs.aws.amazon.com/cli/latest/userguide/cli-configure-quickstart.html#cli-configure-quickstart-creds). Then, create a file `~/.aws/credentials` with the following content:
```
[default]
aws_access_key_id = YOUR_ACCESS_KEY_ID
aws_secret_access_key = YOUR_SECRET_ACCESS_KEY
```
Do not specify any AWS region in that file as the python scripts will allow you to handle multiple regions programmatically.

### Step 2. Add your SSH public key to your AWS account
You must now [add your SSH public key to your AWS account](https://docs.aws.amazon.com/AWSEC2/latest/UserGuide/ec2-key-pairs.html). This operation is manual (AWS exposes little APIs to manipulate keys) and needs to be repeated for each AWS region that you plan to use. Upon importing your key, AWS requires you to choose a 'name' for your key; ensure you set the same name on all AWS regions. This SSH key will be used by the python scripts to execute commands and upload/download files to your AWS instances.
If you don't have an SSH key, you can create one using [ssh-keygen](https://www.ssh.com/ssh/keygen/):
```
$ ssh-keygen -f ~/.ssh/aws
```

### Step 3. Configure the testbed
The file [settings.json](https://github.com/akhilsb/Velox-MPC/blob/master/benchmark/settings.json) (located in [Velox-MPC/benchmarks](https://github.com/akhilsb/Velox-MPC/blob/master/benchmark)) contains all the configuration parameters of the testbed to deploy. Its content looks as follows:
```json
{
    "key": {
        "name": "aws",
        "path": "/absolute/key/path"
    },
    "port": 5000,
    "client_base_port": 7500,
    "client_run_port": 8000,
    "repo": {
        "name": "Velox-MPC",
        "url": "https://github.com/akhilsb/Velox-MPC.git",
        "branch": "master"
    },
    "instances": {
        "type": "c5.large",
        "regions": ["us-east-1"]
    }
}
```
The first block (`key`) contains information regarding your SSH key:
```json
"key": {
    "name": "aws",
    "path": "/absolute/key/path"
},
```
Enter the name of your SSH key; this is the name you specified in the AWS web console in step 2. Also, enter the absolute path of your SSH private key (using a relative path won't work). 


The second block (`ports`) specifies the TCP ports to use:
```json
"port": 5000,
"client_base_port": 7500,
"client_run_port": 8000,
```
The artifact requires a number of TCP ports for communication between the processes. Note that the script will open a large port range (5000-10000) to the LAN on all your AWS instances. 

The third block (`repo`) contains the information regarding the repository's name, the URL of the repo, and the branch containing the code to deploy: 
```json
"repo": {
    "name": "Velox-MPC",
    "url": "https://github.com/akhilsb/Velox-MPC.git",
    "branch": "master"
},
```
Remember to update the `url` field to the name of your repo. Modifying the branch name is particularly useful when testing new functionalities without having to checkout the code locally. 

The the last block (`instances`) specifies the [AWS instance type](https://aws.amazon.com/ec2/instance-types) and the [AWS regions](https://docs.aws.amazon.com/AWSEC2/latest/UserGuide/using-regions-availability-zones.html#concepts-available-regions) to use:
```json
"instances": {
    "type": "c5.large",
    "regions": ["us-east-1"]
}
```
The instance type selects the hardware on which to deploy the testbed. For example, `c5.large` instances come with 2 vCPU (2 physical cores), and 4 GB of RAM. The python scripts will configure each instance with 300 GB of SSD hard drive. The `regions` field specifies the data centers to use. If you require more nodes than data centers, the python scripts will distribute the nodes as equally as possible amongst the data centers. All machines run a fresh install of Ubuntu Server 24.04.

### Step 4. Create a testbed
The AWS instances are orchestrated with [Fabric](http://www.fabfile.org) from the file [fabfile.py](https://github.com/akhil-sb/Velox-MPC/blob/master/benchmark/fabfile.py) (located in [hashrand-rs/benchmarks](https://github.com/akhilsb/Velox-MPC/blob/master/benchmark)); you can list all possible commands as follows:
```bash
fab --list
```
The command `fab create` creates new AWS instances; open [fabfile.py](https://github.com/akhilsb/hashrand-rs/blob/master/benchmark/fabfile.py) and locate the `create` task:
```python
@task
def create(ctx, nodes=n):
    ...
```
The parameter `nodes`, set in the beginning of the `fabfile.py` file, determines how many instances to create in *each* AWS region. That is, if you specified 1 AWS region as in the example of step 3, setting `nodes=16` will create 16 machines:
```bash
fab create

Creating 16 instances |██████████████████████████████| 100.0% 
Waiting for all instances to boot...
Successfully created 16 new instances
```

You can then clone the repo and install rust on the remote instances with `fab install`:
```bash
fab install

Installing rust and cloning the repo...
Initialized testbed of 16 nodes
```

This may take a long time as the command will first update all instances.
The commands `fab stop` and `fab start` respectively stop and start the testbed without destroying it (it is good practice to stop the testbed when not in use as AWS can be quite expensive); and `fab destroy` terminates all instances and destroys the testbed. Note that, depending on the instance types, AWS instances may take up to several minutes to fully start or stop. The command `fab info` displays a nice summary of all available machines and information to manually connect to them (for debug).

### Step 5. Run a benchmark
After setting up the testbed, modify the parameters in the first 4 lines of `fabfile.py` file. Set four parameters:
1. Number of parties `n`
2. Number of messages to broadcast or anonymity set size `k`
3. Batch size - Number of secrets to be batched within each ACSS instance `batch_size`
4. Compression factor - A parameter controlling the tradeoff between rounds and computation in the verification phase. Check the paper for more details. 

But before starting the protocol, create input files containing the messages to broadcast. 
A script has been included in the `inputs/` directory for input generation. 
```bash
cd inputs/
python3 inp_gen.py
```
Then, set the following parameters in `fabfile.py`. 
```python
n = 16
num_messages = 256
batch_size = 1000
compression_factor = 10
```
This command first updates all machines with the latest commit of the GitHub repo and branch specified in the [settings.json](https://github.com/akhilsb/Velox-MPC/blob/master/benchmark/settings.json) (step 3) file; this ensures that benchmarks are always run with the latest version of the code. 
It then generates and uploads the configuration files to each machine, and runs the benchmarks with the specified parameters. Make sure to change the number of nodes in the `remote` function. 
The input parameters for the protocol can be set in the `_config` function in the benchmark/remote.py file in the `benchmark` folder. 

### Step 6: Download logs and Compile results
Download log files after allowing the protocol sufficient time to terminate (Ideally within 2 minutes). 
The following command downloads the log file from the `syncer` titled `syncer-n_{num_parties}_{num_messages}_{batch_size}_{compression_factor}.log` into the `benchmark/logs/` directory. 
```bash
fab logs
```
This file contains the details about the latency of the protocol and the outputs of the nodes. 
If anything goes wrong during a benchmark, you can always stop it by running `fab kill`.
Once this command terminates, cd into the `logs/` directory and run the `stats.py` file to generate results. 
```bash
python3 compile_results.py
```
This command should print out the results as follows. 
```
Average latencies per category:
  Preprocessing: 3440.64 ms
  Online: 3714.00 ms
  verification: 4110.00 ms
  output: 4288.73 ms

Latency differences between subsequent categories:
  Preprocessing → Online: 273.36 ms
  Online → verification: 396.00 ms
  verification → output: 178.73 ms
```

### Step 7: Cleanup
Be sure to kill the prior benchmark using the following command before running a new benchmark. 
Additionally, clean up the files created by the benchmark by running the `cleanup.sh` script.  
```bash
fab kill
./cleanup.sh
```
After running the benchmarks for a given number of nodes, destroy the testbed with the following command. 
```bash
fab destroy
```
This command destroys the testbed and terminates all created AWS instances.
For running a benchmark with a different testbed setup, execute the pipeline from Step 3. 

# Reproducing the results in the paper
To reproduce the results in the paper, run Velox in a LAN testbed (with a single region for instance `us-east-1`) with `n=16,64,112` parties. 
In the `n=16` testbed, run Velox with the following configurations in the `fabfile.py` file.
**Note that we substantially improved our code from the time of submission, so the protocol is a lot faster at these configurations, which result in lower runtimes.** 
1. `num_messages=256`,`batch_size=2000`, `compression_factor=10`
2. `num_messages=512`,`batch_size=2000`, `compression_factor=10`
3. `num_messages=1024`,`batch_size=2000`, `compression_factor=10`

In the `n=64` testbed, run Velox with the following configurations in the `fabfile.py` file. 
1. `num_messages=256`,`batch_size=500`, `compression_factor=10`
2. `num_messages=512`,`batch_size=500`, `compression_factor=10`
3. `num_messages=1024`,`batch_size=500`, `compression_factor=10`

In the `n=112` testbed, run Velox with the following configurations in the `fabfile.py` file. 
1. `num_messages=256`,`batch_size=300`, `compression_factor=10`
2. `num_messages=512`,`batch_size=300`, `compression_factor=10`

## Reproducing results in Figure 1
For reproducing results of Figure 1 in the paper (phase-wise runtime), use the results of the instances with `n=64` parties. 

## Reproducing results in Figure 2
For reproducing results in Figure 2 in the paper (Anonymity set size k vs runtime), use the previous figure's result for `n=64` and add the results of `n=16` parties into the paper.

## Reproducing results in Figure 3
For reproducing results in Figure 3 in the paper (Scalability plot:), add the results with `n=112` parties to the figure. 