// [[file:../magman.note::*docs][docs:1]]
//! This mod is for VASP interactive calculations.
// docs:1 ends here

// [[file:../magman.note::ae9e9435][ae9e9435]]
use super::*;

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use tokio::sync::oneshot;
// ae9e9435 ends here

// [[file:../magman.note::e899191b][e899191b]]
use crate::job::Db;
use crate::job::Job;
use crate::job::JobId;

#[derive(Debug)]
// cmd + working_dir + done?
struct Interaction(String, String, oneshot::Sender<InteractionOutput>);

/// The message sent from client for controlling child process
#[derive(Debug, Clone)]
enum Control {
    Quit,
    Pause,
    Resume,
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
#[derive(Clone)]
struct Session {
    db: Db,
}

impl Session {
    fn new() -> Self {
        Self { db: Db::new() }
    }

    async fn interact(&mut self, int: Interaction) -> Result<InteractionOutput> {
        let Interaction(cmd, wrk_dir, tx_resp) = int;
        let job = create_job_for_remote_session(&cmd, &wrk_dir);
        let out_file = job.out_file.clone();
        let job_id = self.db.insert_job(job).await;
        self.db.wait_job(job_id).await.unwrap();
        let bytes = self.db.get_job_file(job_id, &out_file).await?;
        let o: String = std::str::from_utf8(&bytes)?.into();
        tx_resp.send(o.clone()).unwrap();
        Ok(o)
    }
}

fn create_job_for_remote_session(cmd: &str, wrk_dir: &str) -> Job {
    let script = format!(
        "#! /usr/bin/env bash

cd {wrk_dir}
{cmd}
"
    );

    // FIXME: run in remote node
    let job = Job::new(&script);
    job
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
            log_dbg!();
            // FIXME: refactor
            let (tx_resp, rx_resp) = oneshot::channel();
            log_dbg!();
            self.tx_int.send(Interaction(cmd.into(), wrk_dir.into(), tx_resp)).await?;
            log_dbg!();
            let out = rx_resp.await?;
            log_dbg!();
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
pub struct TaskServer {
    // for receiving interaction message for child process
    rx_int: Option<RxInteraction>,
    // for controlling child process
    rx_ctl: Option<RxControl>,
    // child process
    session: Option<Session>,
}

mod taskserver {
    use super::*;

    impl TaskServer {
        /// Run child process in new session, and serve requests for interactions.
        pub async fn run_and_serve(&mut self) -> Result<()> {
            let mut session = self.session.as_mut().context("no running session")?;
            let mut rx_int = self.rx_int.take().context("no rx_int")?;
            let mut rx_ctl = self.rx_ctl.take().context("no rx_ctl")?;

            let nodes = crate::job::Nodes::new(&["localhost".into(), "hpc44".into()]);

            type JobChan = (Job, tokio::sync::oneshot::Sender<String>);
            let (mut tx_jobs, rx_jobs): (spmc::Sender<JobChan>, spmc::Receiver<JobChan>) = spmc::channel();
            for i in 0.. {
                // make sure run in parallel
                let join_handler = {
                    let session = session.clone();
                    let jobs = rx_jobs.clone();
                    let nodes = nodes.clone();
                    tokio::spawn(async move {
                        log_dbg!();
                        // get node
                        let node = nodes.borrow_node().unwrap();
                        // if let Some(node) = nodes.lock().await.pop() {
                        let (job, tx_resp) = jobs.recv().unwrap();
                        let name = job.name();
                        info!("Starting job {name} ...");
                        let mut compt = job.submit();
                        log_dbg!();
                        compt.start().await.unwrap();
                        log_dbg!();
                        compt.wait().await.unwrap();
                        info!("Job {name} completed, sending stdout to the client ...");
                        tx_resp.send(String::new()).unwrap();
                        log_dbg!();
                        // return node back
                        nodes.return_node(node).unwrap();
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
                        let job = create_job_for_remote_session(&cmd, &wrk_dir);
                        tx_jobs.send((job, tx_resp))?;
                    }
                    Some(ctl) = rx_ctl.recv() => {
                        log_dbg!();
                        match break_control_session(ctl) {
                            Ok(false) => {},
                            Ok(true) => break,
                        Err(err) => {error!("control session error: {:?}", err); break;}
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

    /// Interact with child process: write stdin with `input` and read in stdout by
    /// `read_pattern`
    async fn handle_interaction(session: &mut Session, mut rx_int: RxInteraction, mut rx_ctl: RxControl) -> Result<()> {
        for i in 0.. {
            log_dbg!();
            tokio::select! {
                Some(int) = rx_int.recv() => {
                    log_dbg!();
                    let out = session.interact(int).await?;
                    info!("session completed: {out:?}");
                }
                Some(ctl) = rx_ctl.recv() => {
                    log_dbg!();
                    match break_control_session(ctl) {
                        Ok(false) => {},
                        Ok(true) => break,
                        Err(err) => {error!("control session error: {:?}", err); break;}
                    }
                }
                else => {
                    bail!("Unexpected branch: the communication channels broken?");
                }
            };
        }

        Ok(())
    }

    fn break_control_session(ctl: Control) -> Result<bool> {
        todo!();
    }
}
// 7b4ac45b ends here

// [[file:../magman.note::8408786a][8408786a]]
/// Create task server and client. The client can be cloned and used in
/// concurrent environment
pub fn new_interactive_task() -> (TaskServer, TaskClient) {
    let (tx_int, rx_int) = tokio::sync::mpsc::channel(1);
    let (tx_ctl, rx_ctl) = tokio::sync::mpsc::channel(1);

    let session = Session::new();

    let server = TaskServer {
        rx_int: rx_int.into(),
        rx_ctl: rx_ctl.into(),
        session: session.into(),
    };

    let client = TaskClient {
        tx_int,
        tx_ctl,
    };

    (server, client)
}
// 8408786a ends here
