// [[file:../magman.note::3728ca38][3728ca38]]
//! For handling running task/job
use super::*;

use std::path::{Path, PathBuf};
use gosh::runner::process::Session;
use gosh::runner::prelude::SpawnSessionExt;

use serde::{Deserialize, Serialize};
use tempfile::{tempdir, tempdir_in, TempDir};
// 3728ca38 ends here

// [[file:../magman.note::50e6ed5a][50e6ed5a]]
/// Represents a computational job inputted by user.
#[derive(Debug, Deserialize, Serialize)]
pub struct Job {
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
}
// 50e6ed5a ends here

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

impl Job {
    /// Submit the job and turn it into Computation.
    pub fn submit(self) -> Computation {
        Computation::new(self)
    }
}

impl Computation {
    /// Construct `Computation` of user inputted `Job`.
    pub fn new(job: Job) -> Self {
        use std::fs::File;
        use std::os::unix::fs::OpenOptionsExt;

        // create working directory in scratch space.
        let wdir = tempfile::TempDir::new_in(".").expect("temp dir");
        let session = Computation {
            job,
            wrk_dir: wdir.into(),
            session: None,
        };

        // create run file
        let file = session.run_file();

        // make run script executable
        match std::fs::OpenOptions::new().create(true).write(true).mode(0o770).open(&file) {
            Ok(mut f) => {
                let _ = f.write_all(session.job.script.as_bytes());
                trace!("script content wrote to: {}.", file.display());
            }
            Err(e) => {
                panic!("Error whiling creating job run file: {}", e);
            }
        }
        let file = session.inp_file();
        match File::create(&session.inp_file()) {
            Ok(mut f) => {
                let _ = f.write_all(session.job.input.as_bytes());
                trace!("input content wrote to: {}.", file.display());
            }
            Err(e) => {
                panic!("Error while creating job input file: {}", e);
            }
        }

        session
    }

    /// Wait for background command to complete.
    pub async fn wait(&mut self) -> Result<()> {
        if let Some(s) = self.session.as_mut() {
            let ecode = s.child.wait().await?;
            info!("job session exited: {}", ecode);
            Ok(())
        } else {
            bail!("Job not started yet.");
        }
    }

    /// Run command in background.
    pub async fn start(&mut self) -> Result<()> {
        log_dbg!();
        let wdir = self.wrk_dir();
        info!("job work direcotry: {}", wdir.display());

        let mut session = tokio::process::Command::new(&self.run_file())
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

// [[file:../magman.note::8f205f46][8f205f46]]
mod db {
    use super::*;

    use bytes::Bytes;
    use std::sync::Arc;
    use tokio::sync::Mutex;

    use impl_jobs_slotmap::JobKey;
    use impl_jobs_slotmap::Jobs;

    pub use impl_jobs_slotmap::Id;

    /// A simple in-memory DB for computational jobs.
    #[derive(Clone)]
    pub struct Db {
        inner: Arc<Mutex<Jobs>>,
    }

    impl Db {
        /// Create an empty `Db`
        pub fn new() -> Self {
            Self {
                inner: Arc::new(Mutex::new(Jobs::new())),
            }
        }

        /// Update the job in `id` with a `new_job`. Return error if job `id`
        /// has been started.
        pub async fn update_job(&mut self, id: JobId, new_job: Job) -> Result<()> {
            debug!("update_job: id={}, job={:?}", id, new_job);
            let mut jobs = self.inner.lock().await;
            let k = jobs.check_job(id)?;
            if jobs[k].is_started() {
                bail!("job {} has been started", id);
            } else {
                jobs[k] = new_job.submit();
            }

            Ok(())
        }

        /// Return a full list of submitted jobs
        pub async fn get_job_list(&self) -> Vec<JobId> {
            self.inner.lock().await.iter().map(|(k, _)| k).collect()
        }

        /// Put a new file on working directory of job `id`
        pub async fn put_job_file(&mut self, id: JobId, file: String, body: Bytes) -> Result<()> {
            debug!("put_job_file: id={}", id);

            let jobs = self.inner.lock().await;
            let id = jobs.check_job(id)?;

            let job = &jobs[id];
            let p = job.wrk_dir().join(&file);
            info!("client request to put a file: {}", p.display());
            match std::fs::File::create(p) {
                Ok(mut f) => {
                    f.write_all(&body).context("write job file")?;
                    Ok(())
                }
                Err(e) => {
                    bail!("create file error:\n{}", e);
                }
            }
        }

        /// Return the content of `file` for job `id`
        pub async fn get_job_file(&self, id: JobId, file: &Path) -> Result<Vec<u8>> {
            debug!("get_job_file: id={}", id);
            let jobs = self.inner.lock().await;
            let k = jobs.check_job(id)?;
            let job = &jobs[k];
            let p = job.wrk_dir().join(&file);
            info!("client request file: {}", p.display());

            let mut buffer = Vec::new();
            let _ = std::fs::File::open(p)
                .context("open file")?
                .read_to_end(&mut buffer)
                .context("read file")?;
            Ok(buffer)
        }

        /// List files in working directory of Job `id`.
        pub async fn list_job_files(&self, id: JobId) -> Result<Vec<PathBuf>> {
            info!("list files for job {}", id);
            let jobs = self.inner.lock().await;
            let id = jobs.check_job(id)?;

            let mut list = vec![];
            let job = &jobs[id];
            for entry in std::fs::read_dir(job.wrk_dir()).context("list dir")? {
                if let Ok(entry) = entry {
                    let p = entry.path();
                    if p.is_file() {
                        list.push(p);
                    }
                }
            }
            Ok(list)
        }

        /// Remove all jobs from `Db`. If the job has been started, the child
        /// processes will be terminated.
        pub async fn clear_jobs(&mut self) {
            self.inner.lock().await.clear();
        }

        /// Remove the job `id` from `Db`. If the job has been started, it will
        /// be terminated.
        pub async fn delete_job(&mut self, id: JobId) -> Result<()> {
            info!("delete_job: id={}", id);
            self.inner.lock().await.remove(id)?;
            Ok(())
        }

        /// Insert job into the queue.
        pub async fn insert_job(&mut self, mut job: Job) -> JobId {
            info!("create_job: {:?}", job);
            let mut jobs = self.inner.lock().await;
            let jid = jobs.insert(job.submit());
            info!("Job {} created.", jid);
            jid
        }

        /// Start the job in background, and wait until it finish.
        pub async fn wait_job(&self, id: JobId) -> Result<()> {
            info!("wait_job: id={}", id);
            let mut jobs = self.inner.lock().await;
            let k = jobs.check_job(id)?;
            jobs[k].start().await?;
            jobs[k].wait().await?;
            Ok(())
        }
    }
}
// 8f205f46 ends here

// [[file:../magman.note::*slotmap][slotmap:1]]
mod impl_jobs_slotmap {
    use super::*;

    use bimap::BiMap;
    use slotmap::Key;
    use slotmap::{DefaultKey, SlotMap};

    /// The job `Id` from user side
    pub type Id = usize;

    pub(super) type JobKey = DefaultKey;

    pub struct Jobs {
        inner: SlotMap<DefaultKey, Computation>,
        mapping: BiMap<usize, JobKey>,
    }

    impl Jobs {
        /// Create empty `Jobs`
        pub fn new() -> Self {
            Self {
                inner: SlotMap::new(),
                mapping: BiMap::new(),
            }
        }

        /// Look for the Job with `id`, returning error if the job with `id`
        /// does not exist.
        pub fn check_job(&self, id: Id) -> Result<JobKey> {
            if let Some(&k) = self.mapping.get_by_left(&id) {
                Ok(k)
            } else {
                bail!("Job id not found: {}", id);
            }
        }

        /// Insert a new Job into database, returning Id for later operations.
        pub fn insert(&mut self, job: Computation) -> Id {
            let k = self.inner.insert(job);
            let n = self.mapping.len() + 1;
            if let Err(e) = self.mapping.insert_no_overwrite(n, k) {
                panic!("invalid {:?}", e);
            }
            n
        }

        /// Remove the job with `id`
        pub fn remove(&mut self, id: Id) -> Result<()> {
            let k = self.check_job(id)?;
            let job = &self.inner[k];
            if job.is_started() {
                info!("Job {} has been started.", id);
            }
            // The session will be terminated on drop
            let _ = self.inner.remove(k);
            Ok(())
        }

        /// Remove all created jobs
        pub fn clear(&mut self) {
            for (k, job) in self.inner.iter() {
                if job.is_started() {
                    info!("job {} already started.", self.to_id(k));
                }
            }
            // The session will be terminated on drop
            self.inner.clear();
        }

        /// Iterator over a tuple of `Id` and `Job`.
        pub fn iter(&self) -> impl Iterator<Item = (Id, &Computation)> {
            self.inner.iter().map(move |(k, v)| (self.to_id(k), v))
        }

        fn to_id(&self, k: JobKey) -> Id {
            if let Some(&id) = self.mapping.get_by_right(&k) {
                id
            } else {
                panic!("invalid job key {:?}", k);
            }
        }
    }

    impl std::ops::Index<JobKey> for Jobs {
        type Output = Computation;

        fn index(&self, key: JobKey) -> &Self::Output {
            &self.inner[key]
        }
    }

    impl std::ops::IndexMut<JobKey> for Jobs {
        fn index_mut(&mut self, key: JobKey) -> &mut Self::Output {
            &mut self.inner[key]
        }
    }
}
// slotmap:1 ends here

// [[file:../magman.note::dbe0de29][dbe0de29]]
pub use self::db::Db;
pub use self::db::Id as JobId;
// dbe0de29 ends here
