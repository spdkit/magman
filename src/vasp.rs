// [[file:../magman.note::6445afac][6445afac]]
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use super::*;
use crate::MAG_DB_CONNECTION;

use gosh::db::prelude::*;
// 6445afac ends here

lazy_static! {
    static ref CSV_DATA: HashMap<String, Record> = {
        let filename = "tests/files/results.csv";
        read_data(filename).expect("magresult")
    };
}

// for test
pub struct CsvEvaluator;
impl crate::magmom::EvaluateMagneticState for CsvEvaluator {
    fn evaluate_new(&self, so: &[bool]) -> Result<crate::magmom::MagneticState> {
        let key = crate::magmom::binary_key(so);
        let ms = if let Some(record) = &CSV_DATA.get(&key) {
            let energy = record.energy;
            info!("item {:} energy = {:-18.6}", key, energy);
            crate::magmom::MagneticState::new(so, energy)
        } else {
            bail!("Record not found: {}", crate::magmom::binary_key(so))
        };

        Ok(ms)
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct Record {
    directory: String,
    energy: f64,
    seqs: String,
    net_mag: usize,
}

// read data records from an external csv file
fn read_data(filename: &str) -> Result<HashMap<String, Record>> {
    let mut rdr = csv::Reader::from_path(filename)?;

    let mut data = HashMap::new();
    for result in rdr.deserialize() {
        let record: Record = result?;
        data.insert(record.directory.clone(), record);
    }

    Ok(data)
}

#[test]
#[ignore]
fn test_read_data() {
    let filename = "tests/files/results.csv";
    let x = read_data(filename).expect("magresult");
}

// [[file:../magman.note::f6ae3a4b][f6ae3a4b]]
/// VASP related data
#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
pub struct Vasp {
    /// Command line for running VASP job.
    cmdline: String,

    /// Initial value of MAGMOM for magnetic atom.
    initial_magmom_value: f64,

    /// VASP template directory for calculations of different spin-orderings.
    template_directory: PathBuf,

    /// Working directory for all VASP calculations.
    working_directory: PathBuf,

    /// The placeholder string in INCAR to be replaced by each spin-ordering.
    placeholder_text: String,
}

/// VASP Evaluator
impl crate::magmom::EvaluateMagneticState for Vasp {
    fn evaluate_new(&self, so: &[bool]) -> Result<crate::magmom::MagneticState> {
        let energy = self.calculate_new(so)?;
        let ms = crate::magmom::MagneticState::new(so, energy);
        Ok(ms)
    }
}

impl Default for Vasp {
    fn default() -> Self {
        Self {
            cmdline: "run-vasp.sh".into(),
            template_directory: "template".into(),
            initial_magmom_value: 5.0,
            working_directory: "jobs".into(),
            placeholder_text: "XXXXX".into(),
        }
    }
}

impl Vasp {
    /// Call VASP to calculate energy with spin-ordering of `so`.
    pub(crate) fn calculate_new(&self, so: &[bool]) -> Result<f64> {
        // use gut::cli::duct::cmd;
        use std::process::Command;

        // fix cmdline relative path issue
        let cmdline: &Path = self.cmdline.as_ref();
        let cmdline = cmdline.canonicalize()?;
        let adir = self.job_directory(so);
        debug!("Evaluate job in {adir:?}");
        if !self.already_done(&adir) {
            self.prepare_vasp_inputs(so)?;
            debug!("calculate new job {adir:?} using {cmdline:?}");
            let o = Command::new(&cmdline)
                .current_dir(&adir)
                .output()
                .with_context(|| format!("run {cmdline:?}"))?;
            if !o.status.success() {
                bail!("vasp failed with output: {o:?}");
            }
        }
        let oszicar = adir.join("OSZICAR");
        let energy = get_energy_from_oszicar(oszicar).with_context(|| format!("get energy for {adir:?}"))?;
        println!("job {}, energy = {}", adir.display(), energy);
        Ok(energy)
    }

    /// Collect items from disk files.
    pub(crate) fn collect_results(&self) -> Result<Vec<crate::magmom::MagneticState>> {
        let dir = &self.working_directory;
        let mut list = vec![];
        if dir.is_dir() {
            for entry in std::fs::read_dir(dir)? {
                let entry = entry?;
                let path = entry.path();
                if path.is_dir() {
                    let key = path.file_name().unwrap().to_str().unwrap();
                    let oszicar = path.join("OSZICAR");
                    match get_energy_from_oszicar(oszicar) {
                        Ok(energy) => {
                            println!("job {}, energy = {}", key, energy);
                            let so: Vec<bool> = key
                                .chars()
                                .map(|b| match b {
                                    '1' => true,
                                    '0' => false,
                                    _ => panic!("bad key: {}", key),
                                })
                                .collect();
                            list.push(crate::magmom::MagneticState::new(&so, energy));
                        }
                        Err(e) => {
                            error!("{}", e);
                        }
                    }
                }
            }
        }

        Ok(list)
    }

    /// Inspecting VASP files in disk.
    fn already_done(&self, wdir: &Path) -> bool {
        let incar = wdir.join("INCAR");
        let oszicar = wdir.join("OSZICAR");

        if wdir.is_dir() {
            if incar.is_file() && oszicar.is_file() {
                debug!("Inspecting disk files in {}", wdir.display());
                if let Ok(time2) = oszicar.metadata().and_then(|m| m.modified()) {
                    if let Ok(time1) = incar.metadata().and_then(|m| m.modified()) {
                        if time2 >= time1 {
                            return true;
                        }
                    }
                }
            }
        }

        false
    }

    /// Initial magnetic moment value without considering of spin ordering.
    fn format_as_vasp_tag(&self, so: &[bool]) -> String {
        let ss: Vec<_> = so
            .iter()
            .map(|&spin_up| {
                let v = if spin_up { 1.0 } else { -1.0 } * self.initial_magmom_value;
                format!("{:4.1}", v)
            })
            .collect();
        ss.join(" ")
    }

    /// VASP job directory in spin-ordering `so`.
    fn job_directory(&self, so: &[bool]) -> PathBuf {
        self.working_directory.join(&crate::magmom::binary_key(so))
    }

    /// Prepare VASP input files in working directory.
    fn prepare_vasp_inputs(&self, so: &[bool]) -> Result<()> {
        use std::fs::File;
        use std::io::{BufRead, BufReader};

        let incar = &self.template_directory.join("INCAR");
        let tag = self.placeholder_text.to_uppercase();

        // replace MAGMOM tag
        let mut new_lines = vec![];
        let mut replaced = false;
        for line in
            BufReader::new(File::open(incar).with_context(|| format!("Failed to open VASP INCAR: {}", incar.display()))?).lines()
        {
            let mut line = line?;
            let line_up = line.to_uppercase();
            if line_up.contains("MAGMOM") {
                if line_up.contains(&tag) {
                    let new_tag = self.format_as_vasp_tag(so);
                    line = line_up.replace(&tag, &new_tag);
                    replaced = true;
                }
            }
            new_lines.push(line);
        }
        if !replaced {
            eprintln!("Please fill MAGMOM line in INCAR with {} for templating.", tag);
            bail!("placeholder for setting MAGMOM is not found!");
        }

        // prepare vasp input files
        let poscar = self.template_directory.join("POSCAR");
        let potcar = self.template_directory.join("POTCAR");
        let kpoints = self.template_directory.join("KPOINTS");

        let adir = self.job_directory(so);
        std::fs::create_dir_all(&adir).with_context(|| format!("Failed to create VASP working directory: {}", adir.display()))?;

        let new_incar = &adir.join("INCAR");
        let new_poscar = &adir.join("POSCAR");
        let new_potcar = &adir.join("POTCAR");
        let new_kpoints = &adir.join("KPOINTS");
        gut::fs::write_to_file(new_incar, &new_lines.join("\n"))
            .with_context(|| format!("Failed to write new INCAR file: {}", new_incar.display()))?;

        // use linux symbolic link to reduce disk usage
        fn link_file(src_file: &Path, dst_file: &Path) -> Result<()> {
            use std::os::unix::fs::symlink;

            if dst_file.exists() {
                std::fs::remove_file(dst_file)?;
            }
            // avoid relative path problem.
            let src_file = src_file.canonicalize()?;
            symlink(src_file, dst_file)?;

            Ok(())
        }
        link_file(&poscar, &new_poscar)?;
        link_file(&potcar, &new_potcar)?;
        link_file(&kpoints, &new_kpoints)?;

        Ok(())
    }
}

/// Get energy from vasp OSZICAR file.
fn get_energy_from_oszicar<P: AsRef<Path>>(path: P) -> Result<f64> {
    use std::fs::File;
    use std::io::{BufRead, BufReader};

    // wait up to 2 seconds to make sure OSZICAR to be updated
    let oszicar = path.as_ref();
    let scan_rate = 0.1;
    trace!("read energy from {:?}", oszicar);
    if let Some(line) = gut::fs::read_file(oszicar)?.lines().last() {
        trace!("last line = {}", line);
        for _ in 0..20 {
            if let Some(p) = line.find("E0=") {
                if let Some(s) = line[p + 3..].split_whitespace().next() {
                    let energy = s.parse()?;
                    return Ok(energy);
                }
            }
            trace!("waiting for OSZICAR {:?}", oszicar);
            gut::utils::sleep(scan_rate);
        }
    }

    bail!("Failed to read energy from {}", oszicar.display());
}
// f6ae3a4b ends here

// [[file:../magman.note::*test][test:1]]
#[test]
fn test_get_vasp_energy() -> Result<()> {
    let adir: std::path::PathBuf = "tests/files/jobs/100100001001".into();
    let vasp = Vasp::default();
    assert!(vasp.already_done(&adir));

    let oszicar = adir.join("OSZICAR");
    let e = get_energy_from_oszicar(&oszicar)?;
    assert_eq!(e, -204.12640);

    Ok(())
}

#[test]
fn test_vasp_calculate() -> Result<()> {
    use gut::cli::duct::cmd;

    // setup temp directory
    let dir = tempfile::tempdir()?;

    let mut vasp = Vasp::default();
    vasp.working_directory = dir.path().join("jobs");
    vasp.template_directory = "tests/files/template".into();

    let so = vec![true, true, false, false];
    vasp.prepare_vasp_inputs(&so)?;

    let x = cmd!("ls", "-Rl", dir.path()).read()?;
    print!("{}", x);

    Ok(())
}
// test:1 ends here
