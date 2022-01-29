// [[file:../magman.note::3a532d42][3a532d42]]
use super::*;

use gut::cli::*;
use gut::fs::*;
// 3a532d42 ends here

// [[file:../magman.note::ae28adb7][ae28adb7]]
/// Predict ground-state magnetic ordering of magnetic system.
#[derive(Debug, StructOpt)]
struct MagorderCli {
    #[structopt(flatten)]
    verbose: Verbosity,

    /// The path to VASP OUTCAR
    #[structopt()]
    outcar: PathBuf,
}

pub fn magorder_enter_main() -> Result<()> {
    let args = MagorderCli::from_args();
    args.verbose.setup_logger();

    crate::magorder::validate_magnetization(&args.outcar)?;

    Ok(())
}
// ae28adb7 ends here

// [[file:../magman.note::2ab6d5de][2ab6d5de]]
use crate::remote::{Client, Server};
// 2ab6d5de ends here

// [[file:../magman.note::512e88e7][512e88e7]]
/// A client of a unix domain socket server for interacting with the program
/// run in background
#[derive(Debug, StructOpt)]
struct ClientCli {
    /// Path to the socket file to connect
    #[structopt(short = 'u', default_value = "vasp.sock")]
    socket_file: PathBuf,

    /// Add a new remote node into server
    #[structopt(short)]
    add_node: Option<String>,

    /// The cmd to run in remote session
    #[structopt(long, default_value = "pwd")]
    cmd: String,

    /// The working dir to run the cmd
    #[structopt(long, default_value = ".")]
    wrk_dir: String,
}

impl ClientCli {
    async fn enter_main(self) -> Result<()> {
        // wait a moment for socke file ready
        let timeout = 5;
        wait_file(&self.socket_file, timeout)?;

        let mut stream = Client::connect(&self.socket_file).await?;
        if let Some(node) = self.add_node {
            stream.add_node(node).await?;
        } else {
            stream.interact_with_remote_session(&self.cmd, &self.wrk_dir).await?;
        }

        Ok(())
    }
}
// 512e88e7 ends here

// [[file:../magman.note::674c2404][674c2404]]
/// A helper program to run VASP calculation in remote node
#[derive(Debug, StructOpt)]
struct ServerCli {
    /// Path to the socket file to bind (only valid for interactive calculation)
    #[structopt(default_value = "magman.sock")]
    socket_file: PathBuf,

    /// The remote nodes for calculations
    #[structopt(long, required = true, use_delimiter = true)]
    nodes: Vec<String>,
}

impl ServerCli {
    async fn enter_main(self) -> Result<()> {
        debug!("Run VASP for interactive calculation ...");
        Server::create(&self.socket_file)?.run_and_serve(self.nodes).await?;

        Ok(())
    }
}
// 674c2404 ends here

// [[file:../magman.note::5f9971ad][5f9971ad]]
#[derive(Parser)]
struct Cli {
    #[clap(flatten)]
    verbose: Verbosity,

    #[clap(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Client {
        #[clap(flatten)]
        client: ClientCli,
    },
    Server {
        #[clap(flatten)]
        server: ServerCli,
    },
}

#[tokio::main]
pub async fn remote_enter_main() -> Result<()> {
    let args = Cli::from_args();
    args.verbose.setup_logger();

    match args.command {
        Commands::Client { client } => {
            client.enter_main().await?;
        }
        Commands::Server { server } => {
            debug!("Run VASP for interactive calculation ...");
            server.enter_main().await?;
        }
    }

    Ok(())
}
// 5f9971ad ends here