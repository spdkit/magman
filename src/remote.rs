// [[file:../magman.note::*imports][imports:1]]
use super::*;
use std::process::Command;

use bytes;
use tokio;
// imports:1 ends here

// [[file:../magman.note::6831177b][6831177b]]
/// Shared codes for both server and client sides
mod codec {
    use super::*;
    use bytes::{Buf, BufMut, Bytes};
    use std::io::{Read, Write};
    use tokio::io::{AsyncRead, AsyncReadExt, AsyncWriteExt};
    use tokio::net::UnixStream;

    /// The request from client side
    #[derive(Debug, Eq, PartialEq, Clone)]
    pub enum ServerOp {
        /// Control server process: pause/resume/quit
        Control(Signal),
        /// Request to run command using `cmd_line` in `working_dir`
        Command((String, String)),
    }

    #[derive(Debug, Eq, PartialEq, Clone)]
    pub enum Signal {
        Quit,
        Resume,
        Pause,
    }

    impl ServerOp {
        /// Encode message ready for sent over UnixStream
        pub fn encode(&self) -> Vec<u8> {
            use ServerOp::*;

            let mut buf = vec![];
            match self {
                Control(sig) => {
                    buf.put_u8(b'X');
                    let sig = match sig {
                        Signal::Quit => "SIGTERM",
                        Signal::Resume => "SIGCONT",
                        Signal::Pause => "SIGSTOP",
                    };
                    encode(&mut buf, sig);
                    buf
                }
                Command((input, pattern)) => {
                    buf.put_u8(b'0');
                    encode(&mut buf, input);
                    encode(&mut buf, pattern);
                    buf
                }
                _ => {
                    todo!();
                }
            }
        }

        /// Read and decode raw data as operation for server
        pub async fn decode<R: AsyncRead + std::marker::Unpin>(r: &mut R) -> Result<Self> {
            let mut buf = vec![0_u8; 1];
            r.read_exact(&mut buf).await?;
            let mut buf = &buf[..];

            let op = match buf.get_u8() {
                b'0' => {
                    let cmdline = String::from_utf8_lossy(&decode(r).await?).to_string();
                    let wrk_dir = String::from_utf8_lossy(&decode(r).await?).to_string();
                    ServerOp::Command((cmdline, wrk_dir))
                }
                b'X' => {
                    let sig = String::from_utf8_lossy(&decode(r).await?).to_string();
                    let sig = match sig.as_str() {
                        "SIGTERM" => Signal::Quit,
                        "SIGCONT" => Signal::Resume,
                        "SIGSTOP" => Signal::Pause,
                        _ => todo!(),
                    };
                    ServerOp::Control(sig)
                }
                _ => {
                    todo!();
                }
            };
            Ok(op)
        }
    }

    fn encode<B: BufMut>(mut buf: B, msg: &str) {
        buf.put_u32(msg.len() as u32);
        buf.put(msg.as_bytes());
    }

    async fn decode<R: AsyncRead + std::marker::Unpin>(r: &mut R) -> Result<Vec<u8>> {
        let mut msg = vec![0_u8; 4];
        r.read_exact(&mut msg).await?;
        let mut buf = &msg[..];
        let n = buf.get_u32() as usize;
        let mut msg = vec![0_u8; n];
        r.read_exact(&mut msg).await?;
        Ok(msg)
    }

    pub async fn send_msg(stream: &mut UnixStream, msg: &[u8]) -> Result<()> {
        stream.write_all(msg).await?;
        stream.flush().await?;
        Ok(())
    }

    pub async fn send_msg_encode(stream: &mut UnixStream, msg: &str) -> Result<()> {
        let mut buf = vec![];

        encode(&mut buf, msg);
        send_msg(stream, &buf).await?;

        Ok(())
    }

    pub async fn recv_msg_decode(stream: &mut UnixStream) -> Result<String> {
        let msg = String::from_utf8_lossy(&decode(stream).await?).to_string();
        Ok(msg)
    }

    #[tokio::test]
    async fn test_async_codec() -> Result<()> {
        let op = ServerOp::Control(Signal::Quit);
        let d = op.encode();
        let decoded_op = ServerOp::decode(&mut d.as_slice()).await?;
        assert_eq!(decoded_op, op);

        let cmdline = "echo hello".to_string();
        let wrk_dir = "/tmp/test".to_string();
        let op = ServerOp::Command((cmdline, wrk_dir));
        let d = op.encode();
        let decoded_op = ServerOp::decode(&mut d.as_slice()).await?;
        assert_eq!(decoded_op, op);

        Ok(())
    }
}
// 6831177b ends here

// [[file:../magman.note::03597617][03597617]]
mod client {
    use super::*;
    use gut::fs::*;
    use std::io::{Read, Write};
    use tokio::net::UnixStream;

    /// Client of Unix domain socket
    pub struct Client {
        stream: UnixStream,
    }

    impl Client {
        /// Make connection to unix domain socket server
        pub async fn connect(socket_file: &Path) -> Result<Self> {
            debug!("Connect to socket server: {socket_file:?}");
            let stream = UnixStream::connect(socket_file)
                .await
                .with_context(|| format!("connect to socket file failure: {socket_file:?}"))?;

            let client = Self { stream };
            Ok(client)
        }

        /// Request server to run cmd_line in working_dir and wait until complete.
        pub async fn interact(&mut self, cmd_line: &str, working_dir: &str) -> Result<String> {
            debug!("Request server to run {cmd_line:?} in {working_dir:?} ...");
            let op = codec::ServerOp::Command((cmd_line.into(), working_dir.into()));
            self.send_op(op).await?;

            trace!("receiving output");
            let txt = codec::recv_msg_decode(&mut self.stream).await?;
            trace!("got {} bytes", txt.len());

            Ok(txt)
        }

        /// Try to ask the background computation to stop
        pub async fn try_quit(&mut self) -> Result<()> {
            self.send_op_control(codec::Signal::Quit).await?;

            Ok(())
        }

        /// Try to ask the background computation to pause
        pub async fn try_pause(&mut self) -> Result<()> {
            self.send_op_control(codec::Signal::Pause).await?;

            Ok(())
        }

        /// Try to ask the background computation to resume
        pub async fn try_resume(&mut self) -> Result<()> {
            self.send_op_control(codec::Signal::Resume).await?;

            Ok(())
        }

        /// Send control signal to server process
        async fn send_op_control(&mut self, sig: codec::Signal) -> Result<()> {
            debug!("Send control signal {:?}", sig);
            let op = codec::ServerOp::Control(sig);
            self.send_op(op).await?;

            Ok(())
        }

        async fn send_op(&mut self, op: codec::ServerOp) -> Result<()> {
            use tokio::io::AsyncWriteExt;

            self.stream.write_all(&op.encode()).await?;
            self.stream.flush().await?;

            Ok(())
        }
    }
}
// 03597617 ends here

// [[file:../magman.note::f5489963][f5489963]]
mod server {
    use super::*;
    use interactive::new_interactive_task;
    use interactive::TaskClient;

    use gut::fs::*;
    use tokio::net::{UnixListener, UnixStream};

    /// Computation server backended by unix domain socket
    #[derive(Debug)]
    pub struct Server {
        socket_file: PathBuf,
        listener: UnixListener,
        stream: Option<UnixStream>,
    }

    fn remove_socket_file(s: &Path) -> Result<()> {
        if s.exists() {
            std::fs::remove_file(s)?;
        }

        Ok(())
    }

    impl Server {
        async fn wait_for_client_stream(&mut self) -> Result<UnixStream> {
            let (stream, _) = self.listener.accept().await.context("accept new unix socket client")?;

            Ok(stream)
        }
    }

    impl Drop for Server {
        // clean up existing unix domain socket file
        fn drop(&mut self) {
            let _ = remove_socket_file(&self.socket_file);
        }
    }

    impl Server {
        /// Create a new socket server. Return error if the server already started.
        pub fn create<P: AsRef<Path>>(path: P) -> Result<Self> {
            let socket_file = path.as_ref().to_owned();
            if socket_file.exists() {
                bail!("Socket server already started: {:?}!", socket_file);
            }

            let listener = UnixListener::bind(&socket_file).context("bind socket")?;
            debug!("serve socket {:?}", socket_file);

            Ok(Server {
                listener,
                socket_file,
                stream: None,
            })
        }

        /// Start and serve the client interaction.
        pub async fn run_and_serve(&mut self, nodes: Vec<String>) -> Result<()> {
            // watch for user interruption
            let ctrl_c = tokio::signal::ctrl_c();

            // state will be shared with different tasks
            let (mut server, client) = new_interactive_task();

            let h = server.run_and_serve(nodes);
            tokio::pin!(h);

            tokio::select! {
                _ = ctrl_c => {
                    // it is hard to exit background computations gracefully,
                    // for now we do nothing special here
                    info!("User interrupted. Shutting down ...");
                },
                res = &mut h => {
                    if let Err(e) = res {
                        error!("Task server error: {:?}", e);
                    }
                },
                _ = async {
                    info!("server: start main loop ...");
                    for i in 0.. {
                        // wait for client requests
                        let mut client_stream = self.wait_for_client_stream().await.unwrap();
                        debug!("new incoming connection {}", i);
                        let task = client.clone();
                        // spawn a new task for each client
                        tokio::spawn(async move { handle_client_requests(client_stream, task).await });
                    }
                } => {
                    info!("main loop done?");
                }
            }

            Ok(())
        }
    }

    async fn handle_client_requests(mut client_stream: UnixStream, mut task: TaskClient) {
        use codec::ServerOp;

        while let Ok(op) = ServerOp::decode(&mut client_stream).await {
            match op {
                ServerOp::Command((cmdline, wrk_dir)) => {
                    debug!("client asked to run {cmdline:?} in {wrk_dir:?}");
                    match task.interact(&cmdline, &wrk_dir).await {
                        Ok(txt) => {
                            debug!("sending client text read from stdout");
                            if let Err(err) = codec::send_msg_encode(&mut client_stream, &txt).await {
                                error!("send client msg error: {err:?}");
                            }
                        }
                        Err(err) => {
                            error!("interaction error: {:?}", err);
                        }
                    }
                }
                ServerOp::Control(sig) => {
                    debug!("client sent control signal {:?}", sig);
                    unimplemented!();
                }
                _ => {
                    unimplemented!();
                }
            }
        }
    }
}
// f5489963 ends here

// [[file:../magman.note::0ec87ebc][0ec87ebc]]
pub use client::Client;
pub use server::Server;

impl Client {
    async fn interact_with_remote_session(&mut self, cmd: &str, wrk_dir: &str) -> Result<()> {
        let o = self.interact(cmd, wrk_dir).await?;
        println!("stdout from server side:\n{o}");

        Ok(())
    }
}
// 0ec87ebc ends here

// [[file:../magman.note::251cb9ba][251cb9ba]]
use gut::fs::*;
use structopt::*;

use gut::utils::sleep;

/// Wait until file `f` available for max time of `timeout`.
///
/// # Parameters
/// * timeout: timeout in seconds
/// * f: the file to wait for available
pub fn wait_file(f: &Path, timeout: usize) -> Result<()> {
    // wait a moment for socke file ready
    let interval = 0.1;
    let mut t = 0.0;
    loop {
        if f.exists() {
            trace!("Elapsed time during waiting: {:.2} seconds ", t);
            return Ok(());
        }
        t += interval;
        sleep(interval);

        if t > timeout as f64 {
            bail!("file {:?} doest exist for {} seconds", f, timeout);
        }
    }
}
// 251cb9ba ends here

// [[file:../magman.note::512e88e7][512e88e7]]
/// A client of a unix domain socket server for interacting with the program
/// run in background
#[derive(Debug, StructOpt)]
struct ClientCli {
    #[structopt(flatten)]
    verbose: gut::cli::Verbosity,

    /// Path to the socket file to connect
    #[structopt(short = "u", default_value = "vasp.sock")]
    socket_file: PathBuf,

    /// The cmd to run in remote session
    cmd: String,

    /// The working dir to run the cmd
    #[structopt(long, default_value = ".")]
    wrk_dir: String,
}

#[tokio::main]
pub async fn client_enter_main() -> Result<()> {
    let args = ClientCli::from_args();
    args.verbose.setup_logger();

    // wait a moment for socke file ready
    let timeout = 5;
    wait_file(&args.socket_file, timeout)?;

    Client::connect(&args.socket_file)
        .await?
        .interact_with_remote_session(&args.cmd, &args.wrk_dir)
        .await?;

    Ok(())
}
// 512e88e7 ends here

// [[file:../magman.note::674c2404][674c2404]]
/// A helper program for run VASP calculations
#[derive(Debug, StructOpt)]
struct ServerCli {
    #[structopt(flatten)]
    verbose: gut::cli::Verbosity,

    /// Path to the socket file to bind (only valid for interactive calculation)
    #[structopt(default_value = "magman.sock")]
    socket_file: PathBuf,

    /// The remote nodes for calculations
    #[structopt(long, required = true, use_delimiter=true)]
    nodes: Vec<String>,
}

#[tokio::main]
pub async fn server_enter_main() -> Result<()> {
    let args = ServerCli::from_args();
    args.verbose.setup_logger();

    debug!("Run VASP for interactive calculation ...");
    Server::create(&args.socket_file)?.run_and_serve(args.nodes).await?;

    Ok(())
}
// 674c2404 ends here
