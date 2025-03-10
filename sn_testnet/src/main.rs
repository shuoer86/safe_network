// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

//! safenode provides the interface to Safe routing.  The resulting executable is the node
//! for the Safe network.

#![doc(
    html_logo_url = "https://raw.githubusercontent.com/maidsafe/QA/master/Images/maidsafe_logo.png",
    html_favicon_url = "https://maidsafe.net/img/favicon.ico",
    test(attr(deny(warnings)))
)]
// For explanation of lint checks, run `rustc -W help`.
#![forbid(unsafe_code)]
#![warn(
    missing_debug_implementations,
    missing_docs,
    trivial_casts,
    trivial_numeric_casts,
    unused_extern_crates,
    unused_import_braces,
    unused_qualifications,
    unused_results
)]

use sn_testnet::{Testnet, DEFAULT_NODE_LAUNCH_INTERVAL, FAUCET_BIN_NAME, SAFENODE_BIN_NAME};

use clap::Parser;
use color_eyre::{eyre::eyre, Help, Result};
use std::{
    fs::remove_dir_all,
    io::ErrorKind,
    path::PathBuf,
    process::{Command, Stdio},
};
use tracing::{debug, info};

const DEFAULT_NODE_COUNT: u32 = 25;

// Please do not remove the blank lines in these doc comments.
// They are used for inserting line breaks when the help menu is rendered in the UI.
#[derive(Debug, clap::StructOpt)]
#[clap(name = "testnet", version)]
struct Cmd {
    /// If set, any nodes that are launched will join an existing testnet.
    #[clap(long = "join", short = 'j', value_parser)]
    join_network: bool,

    /// Interval between node launches in milliseconds. Defaults to 5000.
    #[clap(long = "interval", short = 'i')]
    node_launch_interval: Option<u64>,

    /// Use flamegraph setup.
    ///
    /// Flamegraph will elevate to root, so log output will need to be deleted as root.
    ///
    /// Windows is not supported.
    #[clap(long, short = 'f')]
    flame: bool,

    /// Build the node from source.
    ///
    /// This assumes the process is running from the `safe_network` repository.
    #[clap(long, short = 'b')]
    build_node: bool,

    /// Optional path to the safenode binary.
    ///
    /// This will take precedence over the --build-node flag and effectively ignore it.
    ///
    /// If not supplied we will assume that safenode is on PATH.
    #[clap(short = 'p', long, value_name = "FILE_PATH")]
    node_path: Option<PathBuf>,

    /// Build the faucet from source.
    ///
    /// This assumes the process is running from the `safe_network` repository.
    #[clap(long)]
    build_faucet: bool,

    /// Optional path to the faucet binary.
    ///
    /// This will take precedence over the --build_faucet flag and effectively ignore it.
    ///
    /// If not supplied we will assume that faucet is on PATH.
    #[clap(long, value_name = "FILE_PATH")]
    faucet_path: Option<PathBuf>,

    /// The number of nodes for the testnet. Defaults to 30.
    ///
    /// If you use the 'join' command, you must supply this value.
    #[clap(short = 'c', long, env = "NODE_COUNT")]
    node_count: Option<u32>,

    /// Clean the node data directory.
    ///
    /// The data directory location is platform specific:
    ///  - Linux: $HOME/.local/share/safe/node
    ///  - macOS: $HOME/Library/Application Support/safe/node
    ///  - Windows: C:\Users\<username>\AppData\Roaming\safe\node
    ///
    ///  When the `safenode` binary is launched, it creates a 'root' directory under here for each
    ///  particular node that is launched. This directory corresponds to the peer ID that the node
    ///  is assigned.
    ///
    ///  Using this flag will clear all the previous node directories that exist under the data
    ///  directory.
    #[clap(long, verbatim_doc_comment)]
    clean: bool,

    /// Specify any additional arguments to pass to safenode on launch, e.g., --json-logs.
    ///
    /// Any arguments must be valid safenode arguments.
    #[clap(last = true)]
    node_args: Vec<String>,
}

#[tokio::main]
async fn main() -> Result<()> {
    color_eyre::install()?;
    init_tracing()?;

    let args = Cmd::from_args();

    if args.clean {
        let safe_data_dir = dirs_next::data_dir()
            .ok_or_else(|| eyre!("could not obtain root directory path".to_string()))?
            .join("safe");

        println!("Cleaning previous safe directories under {safe_data_dir:?}");
        if let Err(e) = remove_dir_all(safe_data_dir) {
            match e.kind() {
                ErrorKind::NotFound => {
                    println!("No previous safe directories found.");
                }
                _ => {
                    return Err(e.into());
                }
            }
        }
    }

    if args.flame {
        #[cfg(not(target_os = "windows"))]
        check_flamegraph_prerequisites()?;
        #[cfg(target_os = "windows")]
        return Err(eyre!("Flamegraph cannot be used on Windows"));
    }

    let cargo_target_dir = match std::env::var("CARGO_TARGET_DIR") {
        Ok(dir) => {
            let mut dir = PathBuf::from(dir);
            let _ = dir.pop();
            debug!("CARGO_TARGET_DIR is enabled, the path is {dir:?}");
            dir
        }
        Err(_) => PathBuf::new(),
    };

    // build/run node
    let mut node_bin_path = cargo_target_dir.clone();
    if let Some(node_path) = args.node_path {
        node_bin_path.push(node_path);
    } else if args.build_node {
        build_binaries(vec![SAFENODE_BIN_NAME.to_owned()])?;
        node_bin_path.push("target");
        node_bin_path.push("release");
        node_bin_path.push(SAFENODE_BIN_NAME);
    } else {
        node_bin_path.push(SAFENODE_BIN_NAME);
    }

    if args.join_network {
        let node_count = args.node_count.ok_or_else(|| {
            eyre!("A node count must be specified for joining an existing network")
                .suggestion("Please try again using the --node-count argument")
        })?;
        join_network(
            node_bin_path,
            args.node_launch_interval
                .unwrap_or(DEFAULT_NODE_LAUNCH_INTERVAL),
            node_count,
            args.node_args,
        )?;
        return Ok(());
    }

    let gen_multi_addr = run_network(
        node_bin_path,
        args.node_launch_interval
            .unwrap_or(DEFAULT_NODE_LAUNCH_INTERVAL),
        args.node_count.unwrap_or(DEFAULT_NODE_COUNT),
        args.node_args,
        args.flame,
    )
    .await?;

    // build/run faucet
    let mut faucet_bin_path = cargo_target_dir.clone();
    if let Some(faucet_path) = args.faucet_path {
        faucet_bin_path.push(faucet_path);
    } else if args.build_faucet {
        build_binaries(vec![FAUCET_BIN_NAME.to_owned()])?;
        faucet_bin_path.push("target");
        faucet_bin_path.push("release");
        faucet_bin_path.push(FAUCET_BIN_NAME);
    } else {
        faucet_bin_path.push(FAUCET_BIN_NAME);
    }

    info!("Launching CashNote faucet server");
    run_faucet(gen_multi_addr, faucet_bin_path)?;

    println!("Testnet and faucet launched successfully");
    Ok(())
}

#[cfg(not(target_os = "windows"))]
fn check_flamegraph_prerequisites() -> Result<()> {
    let output = Command::new("cargo")
        .arg("install")
        .arg("--list")
        .output()?;
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    if !stdout.contains("flamegraph") {
        return Err(
            eyre!("You do not appear to have an installation of flamegraph")
                .suggestion("Please run 'cargo flamegraph install' and try again"),
        );
    }

    #[cfg(not(target_os = "macos"))]
    {
        let output = Command::new("which").arg("perf").output()?;
        if !output.status.success() {
            return Err(eyre!(
                "You do not appear to have the 'perf' tool installed, which is required for \
                    using flamegraph"
            )
            .suggestion("Please install 'perf' on your OS"));
        }
    }

    Ok(())
}

// Calls cargo build on the given binaries.
fn build_binaries(binaries_to_build: Vec<String>) -> Result<()> {
    let mut args = vec!["build", "--release"];
    for bin in &binaries_to_build {
        args.push("--bin");
        args.push(bin);
    }

    // Keep features consistent to avoid recompiling.
    if cfg!(feature = "chaos") {
        println!("*** Building testnet with CHAOS enabled. Watch out. ***");
        args.push("--features");
        args.push("chaos");
    }
    if cfg!(feature = "statemap") {
        args.extend(["--features", "statemap"]);
    }
    if cfg!(feature = "otlp") {
        args.extend(["--features", "otlp"]);
    }
    if cfg!(feature = "local-discovery") {
        args.extend(["--features", "local-discovery"]);
    }
    if cfg!(feature = "network-contacts") {
        args.extend(["--features", "network-contacts"]);
    }
    if cfg!(feature = "quic") {
        args.extend(["--features", "quic"]);
    }
    if cfg!(feature = "open-metrics") {
        args.extend(["--features", "open-metrics"]);
    }

    let bins_string = binaries_to_build.join(", ");
    info!("Building the following binaries: {bins_string}");

    let mut build_result = Command::new("cargo");
    let _ = build_result.args(args.clone());

    if let Ok(val) = std::env::var("CARGO_TARGET_DIR") {
        let _ = build_result.env("CARGO_TARGET_DIR", val);
    }

    let build_result = build_result
        .current_dir("sn_node")
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .output()?;

    if !build_result.status.success() {
        return Err(eyre!("Failed to build binaries"));
    }

    info!("Binaries built successfully");
    Ok(())
}

/// Start the faucet from the provided bin_path and with the given bootstrap peer
fn run_faucet(gen_multi_addr: String, bin_path: PathBuf) -> Result<()> {
    let testnet = Testnet::configure().node_bin_path(bin_path).build()?;
    let launch_bin = testnet.node_bin_path;

    // server should write logs to a different log dir
    let log_dir = dirs_next::data_dir()
        .ok_or_else(|| eyre!("could not obtain data directory path".to_string()))?
        .join("safe")
        .join("test_faucet")
        .join("server_logs")
        .into_os_string()
        .into_string()
        .map_err(|_| eyre!("Failed get faucet server_log dir"))?;

    let mut args = vec!["--log-output-dest".to_string()];
    args.push(log_dir);
    args.push("--peer".to_string());
    args.push(gen_multi_addr);
    args.push("server".to_string());
    testnet.launcher.launch(&launch_bin, args)?;
    // The launch will immediately complete after fire the cmd out.
    // Have to wait some extra time to allow the faucet to be properly created and funded
    std::thread::sleep(std::time::Duration::from_secs(5));
    Ok(())
}

// Start the network and return the MultiAddr of the genesis node
async fn run_network(
    node_bin_path: PathBuf,
    node_launch_interval: u64,
    node_count: u32,
    mut node_args: Vec<String>,
    flamegraph_mode: bool,
) -> Result<String> {
    let mut testnet = Testnet::configure()
        .node_bin_path(node_bin_path)
        .node_launch_interval(node_launch_interval)
        .flamegraph_mode(flamegraph_mode)
        .build()?;

    let gen_multi_addr = testnet.launch_genesis(node_args.clone()).await?;

    node_args.push("--peer".to_string());
    node_args.push(gen_multi_addr.clone());
    testnet.launch_nodes(node_count as usize, node_args)?;

    sn_testnet::check_testnet::run(&testnet.nodes_dir_path, node_count).await?;

    Ok(gen_multi_addr)
}

fn join_network(
    node_bin_path: PathBuf,
    node_launch_interval: u64,
    node_count: u32,
    node_args: Vec<String>,
) -> Result<()> {
    let mut testnet = Testnet::configure()
        .node_bin_path(node_bin_path)
        .node_launch_interval(node_launch_interval)
        .build()?;
    // The testnet::node_count is set to total_count - 1 to offset for the genesis.
    // Then plus 2 for start. Hence need an offset 1 here.
    testnet.launch_nodes(node_count as usize + 1, node_args)?;
    Ok(())
}

fn init_tracing() -> Result<()> {
    tracing_subscriber::fmt().init();
    Ok(())
}
