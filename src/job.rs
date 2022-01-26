// [[file:../magman.note::3728ca38][3728ca38]]
//! For handling running task/job
use super::*;

use std::path::{Path, PathBuf};
use gosh::runner::process::Session;
use gosh::runner::prelude::SpawnSessionExt;

use tempfile::{tempdir, tempdir_in, TempDir};
// 3728ca38 ends here

// [[file:../magman.note::50e6ed5a][50e6ed5a]]
/// Represents a computational job inputted by user.
#[derive(Debug, Deserialize, Serialize)]
pub struct Job {
    /// A unique random name
    name: String,

    /// Input string for stdin
    input: String,

    /// The content of running script
    script: String,

    /// Path to a file for saving input stream of computation
    pub inp_file: PathBuf,

    /// Path to a file for saving output stream of computation.
    pub out_file: PathBuf,

    /// Path to a file for saving error stream of computation.
    pub err_file: PathBuf,

    /// Path to a script file that defining how to start computation
    pub run_file: PathBuf,

    /// Extra files required for computation
    pub extra_files: Vec<PathBuf>,
}

impl Job {
    /// Construct a Job running shell script.
    ///
    /// # Parameters
    ///
    /// * script: the content of the script for running the job.
    ///
    pub fn new(script: &str) -> Self {
        Self {
            name: random_name(),
            script: script.into(),
            input: String::new(),

            out_file: "job.out".into(),
            err_file: "job.err".into(),
            run_file: "run".into(),
            inp_file: "job.inp".into(),
            extra_files: vec![],
        }
    }

    /// Add a new file into extra-files list.
    pub fn attach_file<P: AsRef<Path>>(&mut self, file: P) {
        let file: PathBuf = file.as_ref().into();
        if !self.extra_files.contains(&file) {
            self.extra_files.push(file);
        } else {
            warn!("try to attach a dumplicated file: {}!", file.display());
        }
    }

    /// Return the job name
    pub fn name(&self) -> String {
        self.name.clone()
    }
}

fn random_name() -> String {
    use rand::distributions::Alphanumeric;
    use rand::Rng;

    let mut rng = rand::thread_rng();
    std::iter::repeat(())
        .map(|()| rng.sample(Alphanumeric))
        .map(char::from)
        .take(6)
        .collect()
}
// 50e6ed5a ends here

// [[file:../magman.note::769262a8][769262a8]]
use crossbeam_channel::{unbounded, Receiver, Sender};

/// Represents a remote node for computation
#[derive(Debug, Clone)]
pub struct Node {
    name: String,
}

impl Node {
    /// Return the name of remote node
    pub fn name(&self) -> &str {
        &self.name
    }
}

impl<T: Into<String>> From<T> for Node {
    fn from(node: T) -> Self {
        let name = node.into();
        assert!(!name.is_empty(), "node name cannot be empty!");
        Self { name }
    }
}

/// Represents a list of remote nodes allocated for computation
#[derive(Clone)]
pub struct Nodes {
    rx: Receiver<Node>,
    tx: Sender<Node>,
}

impl Nodes {
    /// Construct `Nodes` from a list of nodes.
    pub fn new<T: Into<Node>>(nodes: impl IntoIterator<Item = T>) -> Self {
        let (tx, rx) = unbounded();
        let nodes = nodes.into_iter().collect_vec();
        let n = nodes.len();
        info!("We have {n} nodes in totoal for computation.");
        for node in nodes {
            tx.send(node.into()).unwrap();
        }
        Self { rx, tx }
    }

    /// Borrow one node from `Nodes`
    pub fn borrow_node(&self) -> Result<Node> {
        let node = self.rx.recv()?;
        let name = &node.name;
        info!("client borrowed one node: {name:?}");
        Ok(node)
    }

    /// Return one `node` to `Nodes`
    pub fn return_node(&self, node: Node) -> Result<()> {
        let name = &node.name;
        info!("client returned node {name:?}");
        self.tx.send(node)?;
        Ok(())
    }
}
// 769262a8 ends here

// [[file:../magman.note::955c926a][955c926a]]
/// Computation represents a submitted `Job`
pub struct Computation {
    job: Job,

    /// command session. The drop order is above Tempdir
    session: Option<Session<tokio::process::Child>>,

    /// The working directory of computation
    wrk_dir: TempDir,
}
// 955c926a ends here

// [[file:../magman.note::a65e6dae][a65e6dae]]
impl Computation {
    /// The full path to the working directory for running the job.
    pub fn wrk_dir(&self) -> &Path {
        self.wrk_dir.path()
    }

    /// The full path to computation input file (stdin).
    pub fn inp_file(&self) -> PathBuf {
        self.wrk_dir().join(&self.job.inp_file)
    }

    /// The full path to computation output file (stdout).
    pub fn out_file(&self) -> PathBuf {
        self.wrk_dir().join(&self.job.out_file)
    }

    /// The full path to computation error file (stderr).
    pub fn err_file(&self) -> PathBuf {
        self.wrk_dir().join(&self.job.err_file)
    }

    /// The full path to the script for running the job.
    pub fn run_file(&self) -> PathBuf {
        self.wrk_dir().join(&self.job.run_file)
    }
}
// a65e6dae ends here

// [[file:../magman.note::f8672e0c][f8672e0c]]
use tokio::io::AsyncWriteExt;

pub(crate) fn shell_script_for_run_using_ssh(cmd: &str, wrk_dir: &Path, node: &Node) -> String {
    let node_name = node.name();
    let wrk_dir = wrk_dir.shell_escape_lossy();
    let cmd = cmd.shell_escape();

    format!(
        "#! /usr/bin/env bash
ssh -x -o StrictHostKeyChecking=no {node_name} << END
cd {wrk_dir}
{cmd}
END
"
    )
}

impl Job {
    /// Submit the job and turn it into Computation.
    pub fn submit(self) -> Result<Computation> {
        Computation::try_run(self)
    }

    // /// Submit job onto remote `node` using `ssh`.
    // pub fn submit_using_ssh(self, node: &Node) -> Result<Computation> {
    //     let node_name = node.name();
    //     let job_name = self.name();
    //     debug!("run job {job_name:?} on remote node: {node_name:?}");

    //     let comput = Computation::try_run(self)?;
    //     let script = shell_script_for_run_using_ssh(cmdline, wrk_dir, node);

    //     todo!();
    // }
}

fn create_run_file(session: &Computation) -> Result<()> {
    let run_file = session.run_file();
    gut::fs::write_script_file(&run_file, &session.job.script)?;
    wait_file(&run_file, 2)?;

    Ok(())
}

impl Computation {
    /// Construct `Computation` of user inputted `Job`.
    pub fn try_run(job: Job) -> Result<Self> {
        use std::fs::File;

        // create working directory in scratch space.
        let wdir = tempfile::TempDir::new_in(".").expect("temp dir");
        let session = Self {
            job,
            wrk_dir: wdir.into(),
            session: None,
        };

        // create run file and make sure it executable later
        create_run_file(&session)?;
        gut::fs::write_to_file(&session.inp_file(), &session.job.input)?;

        Ok(session)
    }

    /// Wait for background command to complete.
    async fn wait(&mut self) -> Result<()> {
        if let Some(s) = self.session.as_mut() {
            let ecode = s.child.wait().await?;
            info!("job session exited: {}", ecode);
            Ok(())
        } else {
            bail!("Job not started yet.");
        }
    }

    /// Run command in background.
    async fn start(&mut self) -> Result<()> {
        let program = self.run_file();
        let wdir = self.wrk_dir();
        info!("job work direcotry: {}", wdir.display());

        let mut session = tokio::process::Command::new(&program)
            .current_dir(wdir)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn_session()?;

        let mut stdin = session.child.stdin.take().expect("child did not have a handle to stdout");
        let mut stdout = session.child.stdout.take().expect("child did not have a handle to stdout");
        let mut stderr = session.child.stderr.take().expect("child did not have a handle to stderr");

        // NOTE: suppose stdin stream is small.
        stdin.write_all(self.job.input.as_bytes()).await;

        // redirect stdout and stderr to files for user inspection.
        let mut fout = tokio::fs::File::create(self.out_file()).await?;
        let mut ferr = tokio::fs::File::create(self.err_file()).await?;
        tokio::io::copy(&mut stdout, &mut fout).await?;
        tokio::io::copy(&mut stderr, &mut ferr).await?;

        let sid = session.handler().id();
        info!("command running in session {:?}", sid);
        self.session = session.into();

        Ok(())
    }

    /// Start computation, and wait its standard output
    pub async fn run_for_output(&mut self) -> Result<String> {
        if let Err(err) = self.start().await {
            // FIXME: need a better solution
            // try again if due to NFS file synchronous issue
            let err_msg = format!("{:?}", err);
            if err_msg.contains("Text file busy") {
                let run_file = self.run_file();
                warn!("run file {run_file:?} cannot be executed due to file IO issue (Text file busy)");
                info!("Wait 1 second before next trial ...");
                gut::utils::sleep(1.0);
                self.start().await;
            }
        }
        self.wait().await?;
        let txt = gut::fs::read_file(self.out_file())?;
        Ok(txt)
    }

    /// Return true if session already has been started.
    pub fn is_started(&self) -> bool {
        self.session.is_some()
    }
}
// f8672e0c ends here

// [[file:../magman.note::*extra][extra:1]]
impl Computation {
    /// Return a list of full path to extra files required for computation.
    pub fn extra_files(&self) -> Vec<PathBuf> {
        self.job.extra_files.iter().map(|f| self.wrk_dir().join(f)).collect()
    }

    /// Check if job has been done correctly.
    pub fn is_done(&self) -> bool {
        let inpfile = self.inp_file();
        let outfile = self.out_file();
        let errfile = self.err_file();

        if self.wrk_dir().is_dir() {
            if outfile.is_file() && inpfile.is_file() {
                if let Ok(time2) = outfile.metadata().and_then(|m| m.modified()) {
                    if let Ok(time1) = inpfile.metadata().and_then(|m| m.modified()) {
                        if time2 >= time1 {
                            return true;
                        }
                    }
                }
            }
        }

        false
    }

    /// Update file timestamps to make sure `is_done` call return true.
    pub fn fake_done(&self) {
        todo!()
    }
}
// extra:1 ends here

// [[file:../magman.note::47382715][47382715]]
#[test]
#[ignore]
fn test_text_file_busy() -> Result<()> {
    let f = "/home/ybyygu/a";
    if let Err(err) = gut::cli::duct::cmd!(f).read() {
        dbg!(err);
    }
    Ok(())
}
// 47382715 ends here
