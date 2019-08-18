// imports

// [[file:~/Workspace/Programming/structure-predication/magman/magman.note::*imports][imports:1]]
use std::collections::HashMap;
use std::path::Path;

use crate::common::*;
use crate::MAG_DB_CONNECTION;

use gosh_db::prelude::*;
// imports:1 ends here

// csv

// [[file:~/Workspace/Programming/structure-predication/magman/magman.note::*csv][csv:1]]
#[derive(Debug, Serialize, Deserialize, Clone)]
struct Record {
    directory: String,
    energy: f64,
    seqs: String,
    net_mag: usize,
}

impl Collection for Record {
    fn collection_name() -> String {
        "magmom".into()
    }
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
// csv:1 ends here

// calculate

// [[file:~/Workspace/Programming/structure-predication/magman/magman.note::*calculate][calculate:1]]
lazy_static! {
    static ref DATA: HashMap<String, Record> = {
        let filename = "tests/files/results.csv";
        read_data(filename).expect("magresult")
    };
}

impl crate::MagneticState {
    fn prepare_vasp_inputs(&self, config: &crate::Config) -> Result<()> {
        use std::fs::File;
        use std::io::{BufRead, BufReader};

        let incar = config.vasp_job_dir.join("INCAR");
        let tag = config.placeholder_text.to_uppercase();

        // replace MAGMOM tag
        let mut new_lines = vec![];
        let mut replaced = false;
        for line in BufReader::new(File::open(incar)?).lines() {
            let mut line = line?;
            let line_up = line.to_uppercase();
            if line_up.contains("MAGMOM") {
                if line_up.contains(&tag) {
                    line = line_up.replace(&tag, &self.format_as_vasp_tag(config.ini_magmom_value));
                    replaced = true;
                }
            }
            new_lines.push(line);
        }
        if !replaced {
            eprintln!(
                "Please fill MAGMOM line in INCAR with {} for templating.",
                tag
            );
            bail!("placeholder for setting MAGMOM is not found!");
        }

        // prepare vasp input files
        let poscar = config.vasp_job_dir.join("POSCAR");
        let potcar = config.vasp_job_dir.join("POTCAR");
        let kpoints = config.vasp_job_dir.join("KPOINTS");

        let adir: std::path::PathBuf = format!("jobs/{}", self.binary_key()).into();
        let new_incar = adir.join("INCAR");
        let new_poscar = adir.join("POSCAR");
        let new_potcar = adir.join("POTCAR");
        let new_kpoints = adir.join("KPOINTS");
        quicli::fs::write_to_file(new_incar, &new_lines.join("\n"))?;

        // use linux hard link to reduce disk usage
        fn link_file(src_file: &Path, dst_file: &Path) -> Result<()> {
            use std::os::unix::fs::symlink;

            if dst_file.exists() {
                std::fs::remove_file(dst_file)?;
            }
            symlink(src_file, dst_file)?;

            Ok(())
        }
        link_file(&poscar, &new_poscar)?;
        link_file(&potcar, &new_potcar)?;
        link_file(&kpoints, &new_kpoints)?;

        Ok(())
    }

    /// Read in VASP calulcated energy from OSZICAR file.
    fn read_energy_from_oszicar(&mut self) -> Result<()> {
        let file = format!("jobs/{}", self.binary_key());
        let en = get_energy_from_oszicar(file)?;
        self.energy = Some(en);

        Ok(())
    }

    // FIXME: just for test
    pub(crate) fn calculate_new(&mut self) -> Result<()> {
        let key = self.binary_key();

        // self.prepare_vasp_inputs()?;
        // self.submit_vasp()?;
        // self.read_energy_from_oszicar()?;

        let energy = DATA[&key].energy;
        info!("item {:} energy = {:-18.6}", key, energy);
        self.energy = Some(energy);

        Ok(())
    }

    /// Return VASP calulcated energy. If no calculated energy, this function
    /// will submit VASP job.
    pub fn get_energy(&mut self) -> Result<f64> {
        let key = self.binary_key();

        let energy = match Self::get_from_collection(&MAG_DB_CONNECTION, &key) {
            Ok(ms) => ms.energy,
            Err(e) => {
                // FIXME: handle not-found error
                self.calculate_new()?;
                self.put_into_collection(&MAG_DB_CONNECTION, &key)?;
                self.energy
            }
        };

        if let Some(e) = energy {
            Ok(e)
        } else {
            bail!("no calculated energy!");
        }
    }
}

#[test]
fn test_vasp_calculate() -> Result<()> {
    let config = &crate::Config::default();
    let so = vec![true, true, false, false];
    let magmom = crate::MagneticState::new(&so);
    // calculate_vasp_energy(config, &magmom)?;
    Ok(())
}

/// Get energy from vasp OSZICAR file.
fn get_energy_from_oszicar<P: AsRef<Path>>(path: P) -> Result<f64> {
    use std::fs::File;
    use std::io::{BufRead, BufReader};

    let oszicar = path.as_ref();
    if let Some(line) = BufReader::new(File::open(oszicar)?).lines().last() {
        let line = line?;
        if let Some(p) = line.find("E0=") {
            if let Some(s) = line[p + 3..].split_whitespace().next() {
                let energy = s.parse()?;
                return Ok(energy);
            }
        }
    }
    bail!("Failed to get energy from: {}", oszicar.display());
}

#[test]
fn test_get_energy_oszicar() -> Result<()> {
    let fname = "tests/files/OSZICAR";
    let e = get_energy_from_oszicar(fname)?;
    assert_eq!(e, -155.19407);

    Ok(())
}
// calculate:1 ends here
