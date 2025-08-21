# Copyright(C) Facebook, Inc. and its affiliates.
from collections import OrderedDict
from fabric import Connection, ThreadingGroup as Group
from fabric.exceptions import GroupException
from paramiko import RSAKey
from paramiko.ssh_exception import PasswordRequiredException, SSHException
from os.path import basename, splitext
from time import sleep
from math import ceil
from copy import deepcopy
import subprocess
import time

from benchmark.config import Committee, Key, NodeParameters, BenchParameters, ConfigError
from benchmark.utils import BenchError, Print, PathMaker, progress_bar
from benchmark.commands import CommandMaker
from benchmark.logs import LogParser, ParseError
from benchmark.instance import InstanceManager


class FabricError(Exception):
    ''' Wrapper for Fabric exception with a meaningfull error message. '''

    def __init__(self, error):
        assert isinstance(error, GroupException)
        message = list(error.result.values())[-1]
        super().__init__(message)


class ExecutionError(Exception):
    pass


class Bench:
    def __init__(self, ctx):
        self.manager = InstanceManager.make()
        self.settings = self.manager.settings
        try:
            ctx.connect_kwargs.pkey = RSAKey.from_private_key_file(
                self.manager.settings.key_path
            )
            self.connect = ctx.connect_kwargs
        except (IOError, PasswordRequiredException, SSHException) as e:
            raise BenchError('Failed to load SSH key', e)

    def _check_stderr(self, output):
        if isinstance(output, dict):
            for x in output.values():
                if x.stderr:
                    raise ExecutionError(x.stderr)
        else:
            if output.stderr:
                raise ExecutionError(output.stderr)

    def install(self):
        Print.info('Installing rust and cloning the repo...')
        cmd = [
            'sudo apt-get update',
            'sudo apt-get -y upgrade',
            'sudo apt-get -y autoremove',

            # The following dependencies prevent the error: [error: linker `cc` not found].
            'sudo apt-get -y install build-essential',
            'sudo apt-get -y install cmake',
            'sudo apt-get -y install libgmp-dev',

            # Install rust (non-interactive).
            'curl --proto "=https" --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y',
            'source $HOME/.cargo/env',
            'rustup install 1.83.0',
            'rustup override set 1.83.0',

            # This is missing from the Rocksdb installer (needed for Rocksdb).
            'sudo apt-get install -y clang',

            # Clone the repo.
            f'(git clone {self.settings.repo_url} || (cd {self.settings.repo_name} ; git pull))'
        ]
        hosts = self.manager.hosts(flat=True)
        try:
            g = Group(*hosts, user='ubuntu', connect_kwargs=self.connect)
            g.run(' && '.join(cmd), hide=True)
            Print.heading(f'Initialized testbed of {len(hosts)} nodes')
        except (GroupException, ExecutionError) as e:
            e = FabricError(e) if isinstance(e, GroupException) else e
            raise BenchError('Failed to install repo on testbed', e)

    def kill(self, hosts=[], delete_logs=False):
        assert isinstance(hosts, list)
        assert isinstance(delete_logs, bool)
        hosts = hosts if hosts else self.manager.hosts(flat=True)
        delete_logs = CommandMaker.clean_logs() if delete_logs else 'true'
        cmd = [delete_logs, f'({CommandMaker.kill()} || true)']
        try:
            g = Group(*hosts, user='ubuntu', connect_kwargs=self.connect)
            g.run(' && '.join(cmd), hide=True)
        except GroupException as e:
            raise BenchError('Failed to kill nodes', FabricError(e))

    def _select_hosts(self, bench_parameters):
        # Collocate the primary and its workers on the same machine.
        nodes = max(bench_parameters.nodes)

        # Ensure there are enough hosts.
        hosts = self.manager.hosts()
        print("{} {}",sum(len(x) for x in hosts.values()), nodes)
        if sum(len(x) for x in hosts.values()) < nodes:
            return []

        # Select the hosts in different data centers.
        ordered = zip(*hosts.values())
        ordered = [x for y in ordered for x in y]
        return ordered[:nodes]


    def _background_run(self, host, command, log_file):
        name = splitext(basename(log_file))[0]
        cmd = f'tmux new -d -s "{name}" "{command} |& tee {log_file}"'
        c = Connection(host, user='ubuntu', connect_kwargs=self.connect)
        output = c.run(cmd, hide=True)
        self._check_stderr(output)

    def _update(self, hosts):
        ips = list(set(hosts))
        Print.info(
            f'Updating {len(ips)} machines (branch "{self.settings.branch}")...'
        )
        cmd = [
            f'(cd {self.settings.repo_name} && git fetch -f)',
            f'(cd {self.settings.repo_name} && git checkout -f {self.settings.branch})',
            f'(cd {self.settings.repo_name} && git pull -f)',
            'source $HOME/.cargo/env',
            'sudo apt-get install -y pkg-config && sudo apt-get install -y libssl-dev',
            'export RUSTFLAGS="-C target-feature=+aes,+ssse3"',
            f'(cd {self.settings.repo_name} && {CommandMaker.compile()})',
            CommandMaker.alias_binaries(
                f'./{self.settings.repo_name}/target/release/'
            )
        ]
        g = Group(*ips, user='ubuntu', connect_kwargs=self.connect)
        print(g.run(' && '.join(cmd), hide=True))

    def _config(self, hosts, bench_parameters):
        Print.info('Generating configuration files...')
        #print(hosts)
        # Cleanup all local configuration files.
        cmd = CommandMaker.cleanup()
        subprocess.run([cmd], shell=True, stderr=subprocess.DEVNULL)

        # Recompile the latest code.
        cmd = CommandMaker.compile().split()
        print("Running command ",cmd)
        subprocess.run(cmd, check=True, cwd=PathMaker.node_crate_path())

        # Create alias for the client and nodes binary.
        cmd = CommandMaker.alias_binaries(PathMaker.binary_path())
        subprocess.run([cmd], shell=True)

        # Generate the configuration files for Velox. The following code maps the ips of each party. 
        cmd = CommandMaker.generate_config_files(self.settings.base_port,self.settings.client_base_port,self.settings.client_run_port,len(hosts))
        subprocess.run(cmd,shell=True)
        names = [str(x) for x in range(len(hosts))]
        ip_file = ""
        syncer=""
        for x in range(len(hosts)):
            port = self.settings.base_port + x
            syncer_port = self.settings.client_base_port + x
            ip_file += hosts[x]+ ":"+ str(port) + "\n"
            syncer += hosts[x] + ":" + str(syncer_port) + "\n"
        ip_file += hosts[0] + ":" + str(self.settings.client_run_port) + "\n"
        with open("ip_file", 'w') as f:
            f.write(ip_file)
        f.close()
        with open("syncer",'w') as f:
            f.write(syncer)
        f.close()
        #names = [str(x) for x in range(len(hosts))]
        addresses = OrderedDict(
            (x, [y] * (1 + 1)) for x, y in zip(names, hosts)
        )
        # if bench_parameters.collocate:
        #     workers = bench_parameters.workers
        
        # else:
        
        committee = Committee(addresses, self.settings.base_port)
        committee.print(PathMaker.committee_file())

        # start the syncer on the first node first. 
        # Cleanup all nodes and upload configuration files.
        names = names[:len(names)-bench_parameters.faults]
        progress = progress_bar(names, prefix='Uploading config files:')
        for i,name in enumerate(progress):
            #for ip in committee.ips(name):
            c = Connection(hosts[i], user='ubuntu', connect_kwargs=self.connect)
            c.run(f'{CommandMaker.cleanup()} || true', hide=True)
            #c.put(PathMaker.committee_file(), '.')
            if i == 0:
                print('Node 0: writing syncer')
                c.put(PathMaker.syncer(),'.')
            # Write the configuration file to the remote machine. 
            c.put(PathMaker.key_file(i), '.')
            # Write the input file to the remote machine. 
            c.put(PathMaker.input_file(i),'.')
            # Write the ip_file to the remote machine. 
            c.put("ip_file",'.')
            
        Print.info('Booting primaries...')

        for i,ip in enumerate(hosts):
            #host = Committee.ip(address)
            if i == 0:
                # Run syncer first
                print('Running syncer')
                cmd = CommandMaker.run_syncer(
                    PathMaker.key_file(i),
                    bench_parameters.num_messages,
                    bench_parameters.batch_size,
                    bench_parameters.compression_factor
                )
                print('Running the following command on the remote machine:', cmd)
                log_file = PathMaker.syncer_log_file()
                self._background_run(ip, cmd, log_file)
            cmd = CommandMaker.run_primary(
                PathMaker.key_file(i),
                bench_parameters.num_messages,
                bench_parameters.batch_size,
                bench_parameters.compression_factor
            )
            log_file = PathMaker.primary_log_file(i)
            self._background_run(ip, cmd, log_file)
        return committee

    def _just_run(self, hosts, bench_parameters):
        # While calling this method, the configuration files have already been written to the remote machines. 
        Print.info('Booting primaries...')

        for i,ip in enumerate(hosts):
            if i == 0:
                # Run syncer first
                print('Running syncer')
                cmd = CommandMaker.run_syncer(
                    PathMaker.key_file(i),
                    bench_parameters.num_messages,
                    bench_parameters.batch_size,
                    bench_parameters.compression_factor
                )
                print(cmd)
                log_file = PathMaker.syncer_log_file()
                self._background_run(ip, cmd, log_file)
            cmd = CommandMaker.run_primary(
                PathMaker.key_file(i),
                bench_parameters.num_messages,
                bench_parameters.batch_size,
                bench_parameters.compression_factor
            )
            log_file = PathMaker.primary_log_file(i)
            self._background_run(ip, cmd, log_file)

    def _logs(self, hosts, faults, bench_parameters):
        # Delete local logs (if any).
        cmd = CommandMaker.clean_logs()
        subprocess.run([cmd], shell=True, stderr=subprocess.DEVNULL)

        # Download log files.
        #workers_addresses = committee.workers_addresses(faults)
        progress = progress_bar(hosts, prefix='Downloading workers logs:')
        for i, address in enumerate(progress):
            if i==0:
                c = Connection(address, user='ubuntu', connect_kwargs=self.connect)
                c.get(
                    PathMaker.syncer_log_file(),
                    local=PathMaker.syncer_local_log_file(
                        bench_parameters.nodes[0],
                        bench_parameters.num_messages,
                        bench_parameters.batch_size,
                        bench_parameters.compression_factor
                    )
                )
                c.get(
                    PathMaker.client_log_file(i), 
                    local= PathMaker.client_local_log_file(
                        i,
                        bench_parameters.nodes[0],
                        bench_parameters.num_messages,
                        bench_parameters.batch_size,
                        bench_parameters.compression_factor
                    )
                )

        Print.info('Downloaded logs from server, check latencies in the logs folder')

    def run(self, bench_parameters_dict, debug=False):
        assert isinstance(debug, bool)
        Print.heading('Starting remote benchmark')
        try:
            bench_parameters = BenchParameters(bench_parameters_dict)
        except ConfigError as e:
            raise BenchError('Invalid nodes or bench parameters', e)

        # Select which hosts to use.
        selected_hosts = self._select_hosts(bench_parameters)
        print("IP Addresses of selected machines: ", selected_hosts)
        if not selected_hosts:
            Print.warn('There are not enough instances available')
            return

        # Update nodes.
        try:
            self._update(selected_hosts)
        except (GroupException, ExecutionError) as e:
            e = FabricError(e) if isinstance(e, GroupException) else e
            raise BenchError('Failed to update nodes', e)

        # Upload all configuration files and run the protocol. 
        try:
            committee = self._config(
                selected_hosts, bench_parameters
            )
        except (subprocess.SubprocessError, GroupException) as e:
            e = FabricError(e) if isinstance(e, GroupException) else e
            raise BenchError('Failed to configure nodes', e)

    def justrun(self, bench_parameters_dict, debug=False):
        assert isinstance(debug, bool)
        Print.heading('Starting remote benchmark')
        try:
            bench_parameters = BenchParameters(bench_parameters_dict)
        except ConfigError as e:
            raise BenchError('Invalid nodes or bench parameters', e)

        # Select which hosts to use.
        selected_hosts = self._select_hosts(bench_parameters)
        print("IP addresses of selected hosts:", selected_hosts)
        if not selected_hosts:
            Print.warn('There are not enough instances available')
            return

        # Update nodes.
        try:
            self._update(selected_hosts)
        except (GroupException, ExecutionError) as e:
            e = FabricError(e) if isinstance(e, GroupException) else e
            raise BenchError('Failed to update nodes', e)

        # Upload all configuration files.
        try:
            committee = self._just_run(
                selected_hosts, bench_parameters
            )
        except (subprocess.SubprocessError, GroupException) as e:
            e = FabricError(e) if isinstance(e, GroupException) else e
            raise BenchError('Failed to configure nodes', e)

    def pull_logs(self, bench_parameters_dict, debug=False):
        assert isinstance(debug, bool)
        Print.heading('Starting remote benchmark')
        try:
            bench_parameters = BenchParameters(bench_parameters_dict)
        except ConfigError as e:
            raise BenchError('Invalid nodes or bench parameters', e)

        # Select which hosts to use.
        selected_hosts = self._select_hosts(bench_parameters)
        self._logs(selected_hosts,0, bench_parameters)
