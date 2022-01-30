// [[file:../magman.note::b8081727][b8081727]]
use super::*;

use warp::Filter;
// b8081727 ends here

// [[file:../magman.note::50e6ed5a][50e6ed5a]]
/// Represents a computational job inputted by user.
#[derive(Debug, Deserialize, Serialize)]
#[serde(default)]
pub struct Job {
    /// The content of running script
    script: String,

    /// A unique random name
    name: String,

    /// Path to a file for saving output stream of computation.
    pub out_file: PathBuf,

    /// Path to a file for saving error stream of computation.
    pub err_file: PathBuf,

    /// Path to a script file that defining how to start computation
    pub run_file: PathBuf,
}

impl Default for Job {
    fn default() -> Self {
        Self {
            script: "pwd".into(),
            name: random_name(),
            out_file: "job.out".into(),
            err_file: "job.err".into(),
            run_file: "run".into(),
        }
    }
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
            script: script.into(),
            ..Default::default()
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

// [[file:../magman.note::e19bce71][e19bce71]]
use std::path::{Path, PathBuf};

use gosh::runner::prelude::SpawnSessionExt;
use gosh::runner::process::Session;

use tempfile::{tempdir, tempdir_in, TempDir};
use tokio::io::AsyncWriteExt;
// e19bce71 ends here

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
    fn wrk_dir(&self) -> &Path {
        self.wrk_dir.path()
    }

    /// The full path to computation output file (stdout).
    fn out_file(&self) -> PathBuf {
        self.wrk_dir().join(&self.job.out_file)
    }

    /// The full path to computation error file (stderr).
    fn err_file(&self) -> PathBuf {
        self.wrk_dir().join(&self.job.err_file)
    }

    /// The full path to the script for running the job.
    fn run_file(&self) -> PathBuf {
        self.wrk_dir().join(&self.job.run_file)
    }
}
// a65e6dae ends here

// [[file:../magman.note::f8672e0c][f8672e0c]]
impl Job {
    /// Submit the job and turn it into Computation.
    pub fn submit(self) -> Result<Computation> {
        Computation::try_run(self)
    }
}

impl Computation {
    /// create run file and make sure it executable later
    fn create_run_file(&self) -> Result<()> {
        let run_file = &self.run_file();
        gut::fs::write_script_file(run_file, &self.job.script)?;
        wait_file(&run_file, 2)?;

        Ok(())
    }

    /// Construct `Computation` of user inputted `Job`.
    fn try_run(job: Job) -> Result<Self> {
        use std::fs::File;

        // create working directory in scratch space.
        let wdir = tempfile::TempDir::new_in(".").expect("temp dir");
        let session = Self {
            job,
            wrk_dir: wdir.into(),
            session: None,
        };

        session.create_run_file()?;

        Ok(session)
    }

    /// Wait for background command to complete.
    async fn wait(&mut self) -> Result<()> {
        if let Some(s) = self.session.as_mut() {
            let ecode = s.child.wait().await?;
            info!("job session exited: {}", ecode);
            if !ecode.success() {
                error!("job exited unsuccessfully!");
                let txt = gut::fs::read_file(self.run_file())?;
                let run = format!("run file: {txt:?}");
                let txt = gut::fs::read_file(self.err_file())?;
                let err = format!("stderr: {txt:?}");
                bail!("Job failed with error:\n{run:?}{err:?}");
            }
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
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn_session()?;

        let mut stdout = session.child.stdout.take().expect("child did not have a handle to stdout");
        let mut stderr = session.child.stderr.take().expect("child did not have a handle to stderr");

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

    /// Start computation, and wait and return its standard output
    pub async fn wait_for_output(&mut self) -> Result<String> {
        self.start().await?;
        self.wait().await?;
        let txt = gut::fs::read_file(self.out_file())?;
        Ok(txt)
    }

    /// Return true if session already has been started.
    fn is_started(&self) -> bool {
        self.session.is_some()
    }
}
// f8672e0c ends here

// [[file:../magman.note::34c67980][34c67980]]
impl Computation {
    /// Check if job has been done correctly.
    fn is_done(&self) -> bool {
        let runfile = self.run_file();
        let outfile = self.out_file();

        if self.wrk_dir().is_dir() {
            if outfile.is_file() && runfile.is_file() {
                if let Ok(time2) = outfile.metadata().and_then(|m| m.modified()) {
                    if let Ok(time1) = runfile.metadata().and_then(|m| m.modified()) {
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
    fn fake_done(&self) {
        todo!()
    }
}
// 34c67980 ends here

// [[file:../magman.note::08048436][08048436]]
use std::sync::atomic;

static SERVER_BUSY: atomic::AtomicBool = atomic::AtomicBool::new(false);

fn server_busy() -> bool {
    SERVER_BUSY.load(atomic::Ordering::SeqCst)
}

fn server_mark_busy() {
    if !server_busy() {
        SERVER_BUSY.store(true, atomic::Ordering::SeqCst);
    } else {
        panic!("server is already busy")
    }
}

fn server_mark_free() {
    if server_busy() {
        SERVER_BUSY.store(false, atomic::Ordering::SeqCst);
    } else {
        panic!("server is already free")
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
enum ComputationResult {
    JobCompleted(String),
    JobFailed(String),
    NotStarted(String),
}
// 08048436 ends here

// [[file:../magman.note::07c5146c][07c5146c]]
mod handlers {
    use super::*;

    /// POST /jobs with JSON body
    pub async fn create_job(job: Job) -> Result<impl warp::Reply, warp::Rejection> {
        if !server_busy() {
            server_mark_busy();
            let comput = job.submit();
            match comput {
                Ok(mut comput) => match comput.wait_for_output().await {
                    Ok(out) => {
                        server_mark_free();
                        let ret = ComputationResult::JobCompleted(out);
                        Ok(warp::reply::json(&ret))
                    }
                    Err(err) => {
                        server_mark_free();
                        let msg = format!("{err:?}");
                        let ret = ComputationResult::JobFailed(msg);
                        Ok(warp::reply::json(&ret))
                    }
                },
                Err(err) => {
                    server_mark_free();
                    let msg = format!("failed to create job: {err:?}");
                    error!("{msg}");
                    let ret = ComputationResult::JobFailed(msg);
                    Ok(warp::reply::json(&ret))
                }
            }
        } else {
            server_mark_free();
            let msg = format!("Server is busy");
            let ret = ComputationResult::NotStarted(msg);
            Ok(warp::reply::json(&ret))
        }
    }
}
// 07c5146c ends here

// [[file:../magman.note::a5b61fa9][a5b61fa9]]
mod filters {
    use super::*;

    fn json_body() -> impl Filter<Extract = (Job,), Error = warp::Rejection> + Clone {
        warp::body::content_length_limit(1024 * 3200).and(warp::body::json())
    }

    /// POST /jobs with JSON body
    async fn job_run() -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
        warp::path!("jobs")
            .and(warp::post())
            .and(json_body())
            .and_then(handlers::create_job)
    }

    pub async fn api() -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
        job_run().await
    }
}
// a5b61fa9 ends here

// [[file:../magman.note::1dd5d4ed][1dd5d4ed]]
mod server {
    use super::*;
    use std::fmt::Debug;
    use std::net::{SocketAddr, ToSocketAddrs};

    /// Computation server.
    pub struct Server {
        pub address: SocketAddr,
    }

    impl Server {
        pub fn new(addr: impl ToSocketAddrs + Debug) -> Self {
            let addrs: Vec<_> = addr.to_socket_addrs().expect("bad address").collect();
            assert!(addrs.len() > 0, "invalid server address: {addr:?}");
            Self { address: addrs[0] }
        }
    }
}
// 1dd5d4ed ends here

// [[file:../magman.note::e324852d][e324852d]]
/// Submit job remotely using REST api service
pub struct RemoteComputation {
    job: Job,
    client: reqwest::blocking::Client,
    service_uri: String,
}

impl RemoteComputation {
    pub async fn wait_for_output(&self) -> Result<String> {
        let resp = self
            .client
            .post(&self.service_uri)
            .json(&self.job)
            .send()?
            .text()
            .context("client requests to create job")?;
        Ok(resp)
    }
}

impl Job {
    /// Remote submission using RESTful service
    pub fn submit_remote(self, server_address: &str) -> Result<RemoteComputation> {
        // NOTE: the default request timeout is 30 seconds. Here we disable
        // timeout using reqwest builder.
        let client = reqwest::blocking::Client::builder().timeout(None).build()?;
        let uri = format!("http://{}/jobs/", server_address);
        let comput = RemoteComputation {
            job: self,
            service_uri: uri,
            client,
        };

        Ok(comput)
    }
}
// e324852d ends here

// [[file:../magman.note::62b9ac23][62b9ac23]]
impl server::Server {
    pub async fn bind(addr: &str) {
        let server = Self::new(addr);
        server.serve().await;
    }

    async fn serve(&self) {
        let (tx, rx) = tokio::sync::oneshot::channel();
        let services = warp::serve(filters::api().await);
        let (addr, server) = services.bind_with_graceful_shutdown(self.address, async {
            rx.await.ok();
        });
        println!("listening on {addr:?}");

        let ctrl_c = tokio::signal::ctrl_c();
        tokio::select! {
            _ = server => {
                eprintln!("server closed");
            }
            _ = ctrl_c => {
                let _ = tx.send(());
                eprintln!("user interruption");
            }
        }
    }
}
// 62b9ac23 ends here

// [[file:../magman.note::e91e1d87][e91e1d87]]
pub mod cli {
    use super::*;
    use gut::cli::*;

    /// Application server for remote calculations.
    #[derive(StructOpt, Debug)]
    struct Cli {
        #[structopt(flatten)]
        verbose: gut::cli::Verbosity,

        /// Set application server address for binding.
        ///
        /// * Example
        ///
        /// - app-server localhost:3030 (default)
        /// - app-server tower:7070
        #[structopt(name = "ADDRESS", default_value = "localhost:3030")]
        address: String,
    }

    #[tokio::main]
    pub async fn server_enter_main() -> Result<()> {
        let args = Cli::from_args();
        args.verbose.setup_logger();
        server::Server::bind(&args.address).await;

        Ok(())
    }
}
// e91e1d87 ends here

// [[file:../magman.note::27b117b8][27b117b8]]
#[cfg(test)]
mod tests {
    use super::*;
    use warp::test::request;

    #[tokio::test]
    async fn test_warp_post() {
        let api = filters::api().await;
        let resp = request().method("POST").path("/jobs").json(&job_pwd()).reply(&api).await;
        assert!(resp.status().is_success());
        let x: ComputationResult = serde_json::from_slice(&resp.body()).unwrap();
        assert_eq!(x, ComputationResult::JobCompleted("/tmp\n".into()));
    }

    fn job_pwd() -> Job {
        let job = Job::new("cd /tmp; pwd");
        let x = serde_json::to_string(&job);
        dbg!(x);
        job
    }
}
// 27b117b8 ends here
