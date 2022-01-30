// [[file:../magman.note::*docs][docs:1]]
//! This mod is for VASP interactive calculations.
// docs:1 ends here

// [[file:../magman.note::ae9e9435][ae9e9435]]
use super::*;
// use crate::job::Job;
use crate::restful::Job;

use tokio::sync::oneshot;
// ae9e9435 ends here

// [[file:../magman.note::e899191b][e899191b]]
#[derive(Debug)]
// cmd + working_dir + done?
struct Interaction(String, String, oneshot::Sender<InteractionOutput>);

/// The message sent from client for controlling child process
#[derive(Debug, Clone)]
enum Control {
    Quit,
    Pause,
    Resume,
    AddNode(String),
}

type InteractionOutput = String;
type RxInteractionOutput = tokio::sync::watch::Receiver<InteractionOutput>;
type TxInteractionOutput = tokio::sync::watch::Sender<InteractionOutput>;
type RxInteraction = tokio::sync::mpsc::Receiver<Interaction>;
type TxInteraction = tokio::sync::mpsc::Sender<Interaction>;
type RxControl = tokio::sync::mpsc::Receiver<Control>;
type TxControl = tokio::sync::mpsc::Sender<Control>;
// e899191b ends here

// [[file:../magman.note::de5d8bd5][de5d8bd5]]
use crate::job::Node;

fn shell_script_for_job(cmd: &str, wrk_dir: &std::path::Path, node: &Node) -> String {
    let node_name = node.name();
    let wrk_dir = wrk_dir.shell_escape_lossy();

    format!(
        "#! /usr/bin/env bash
cd {wrk_dir}
{cmd}
"
    )
}

fn create_job_for_remote_session(cmd: &str, wrk_dir: &str, node: &Node) -> Job {
    debug!("run cmd {cmd:?} on remote node: {node:?}");
    let script = shell_script_for_job(cmd, wrk_dir.as_ref(), node);

    Job::new(&script)
}
// de5d8bd5 ends here

// [[file:../magman.note::d88217da][d88217da]]
#[derive(Clone)]
/// Manage client requests in threading environment
pub struct TaskClient {
    // for send client request for pause, resume, stop computation on server side
    tx_ctl: TxControl,
    // for interaction with child process on server side
    tx_int: TxInteraction,
}

mod taskclient {
    use super::*;

    impl TaskClient {
        pub async fn interact(&mut self, cmd: &str, wrk_dir: &str) -> Result<String> {
            // FIXME: refactor
            let (tx_resp, rx_resp) = oneshot::channel();
            self.tx_int.send(Interaction(cmd.into(), wrk_dir.into(), tx_resp)).await?;
            let out = rx_resp.await?;
            Ok(out)
        }

        /// Request the server to pause computation
        pub async fn pause(&self) -> Result<()> {
            trace!("send pause task msg");
            self.tx_ctl.send(Control::Pause).await?;
            Ok(())
        }

        /// Request the server to resume computation
        pub async fn resume(&self) -> Result<()> {
            trace!("send resume task msg");
            self.tx_ctl.send(Control::Resume).await?;
            Ok(())
        }

        /// Add one remote node into list for computation
        pub async fn add_node(&self, node: String) -> Result<()> {
            trace!("send add_node ctl msg");
            self.tx_ctl.send(Control::AddNode(node)).await?;
            Ok(())
        }

        /// Request the server to terminate computation
        pub async fn terminate(&self) -> Result<()> {
            trace!("send quit task msg");
            self.tx_ctl.send(Control::Quit).await?;
            Ok(())
        }
    }
}
// d88217da ends here

// [[file:../magman.note::7b4ac45b][7b4ac45b]]
use crate::job::Nodes;

pub struct TaskServer {
    // for receiving interaction message for child process
    rx_int: Option<RxInteraction>,
    // for controlling child process
    rx_ctl: Option<RxControl>,
}

type Jobs = (String, String, oneshot::Sender<String>);
type RxJobs = spmc::Receiver<Jobs>;
type TxJobs = spmc::Sender<Jobs>;
async fn handle_client_interaction(jobs: RxJobs, node: &Node) -> Result<()> {
    let (cmd, wrk_dir, tx_resp) = jobs.recv()?;
    let job = create_job_for_remote_session(&cmd, &wrk_dir, &node);
    let name = job.name();
    info!("Start computing job {name} ...");
    // FIXME: remote or local submission, make it selectable
    // let mut comput = job.submit()?;
    let mut comput = job.submit_remote(node.name())?;
    // if computation failed, we should tell the client to exit
    match comput.wait_for_output().await {
        Ok(out) => {
            info!("Job {name} completed, sending stdout to the client ...");
            if let Err(_) = tx_resp.send(out) {
                error!("the client has been dropped");
            }
        }
        Err(err) => {
            error!("Job {name:?} failed with error: {err:?}");
            tx_resp.send("Job failure".into()).ok();
        }
    }

    Ok(())
}

mod taskserver {
    use super::*;

    impl TaskServer {
        /// Run child process in new session, and serve requests for interactions.
        pub async fn run_and_serve(&mut self, nodes: Vec<String>) -> Result<()> {
            let mut rx_int = self.rx_int.take().context("no rx_int")?;
            let mut rx_ctl = self.rx_ctl.take().context("no rx_ctl")?;

            let nodes = Nodes::new(nodes);
            let (mut tx_jobs, rx_jobs) = spmc::channel();
            for i in 0.. {
                // make sure run in parallel
                let join_handler = {
                    let jobs = rx_jobs.clone();
                    let nodes = nodes.clone();
                    tokio::spawn(async move {
                        match nodes.borrow_node() {
                            Ok(node) => {
                                if let Err(err) = handle_client_interaction(jobs, &node).await {
                                    error!("found error when running job: {err:?}");
                                }
                                // return node back
                                if let Err(err) = nodes.return_node(node) {
                                    error!("found error when return node: {err:?}");
                                }
                            }
                            Err(err) => {
                                error!("found error when borrowing node: {err:?}");
                            }
                        }
                    })
                };
                // handle logic in main thread
                tokio::select! {
                    Ok(_) = join_handler => {
                        log_dbg!();
                    }
                    Some(int) = rx_int.recv() => {
                        log_dbg!();
                        let Interaction(cmd, wrk_dir, tx_resp) = int;
                        tx_jobs.send((cmd, wrk_dir, tx_resp))?;
                    }
                    Some(ctl) = rx_ctl.recv() => {
                        log_dbg!();
                        match ctl {
                            Control::AddNode(node) => {
                                info!("client asked to add a new remote node: {node:?}");
                                let nodes = nodes.clone();
                                nodes.return_node(node.into());
                            }
                            _ => {
                                log_dbg!();
                            }
                        }
                    }
                    else => {
                        bail!("Unexpected branch: the communication channels broken?");
                    }
                }
            }
            Ok(())
        }
    }
}
// 7b4ac45b ends here

// [[file:../magman.note::8408786a][8408786a]]
/// Create task server and client. The client can be cloned and used in
/// concurrent environment
pub fn new_interactive_task() -> (TaskServer, TaskClient) {
    let (tx_int, rx_int) = tokio::sync::mpsc::channel(1);
    let (tx_ctl, rx_ctl) = tokio::sync::mpsc::channel(1);

    let server = TaskServer {
        rx_int: rx_int.into(),
        rx_ctl: rx_ctl.into(),
    };

    let client = TaskClient {
        tx_int,
        tx_ctl,
    };

    (server, client)
}
// 8408786a ends here
