// config.rs
// :PROPERTIES:
// :header-args: :tangle src/config.rs
// :END:

// [[file:~/Workspace/Programming/structure-predication/magman/magman.note::*config.rs][config.rs:1]]
use serde::*;
use toml;

#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
pub struct Config {
    /// VASP template directory for calculations of different spin-orderings.
    pub vasp_job_dir: std::path::PathBuf,

    /// Number of magnetic atoms, such as Fe, Co, Ni, ...
    pub num_magnetic_atoms: usize,

    /// Initial value of MAGMOM for magnetic atom.
    pub ini_magmom_value: f64,

    /// The placeholder string in INCAR to be replaced by each spin-ordering.
    pub placeholder_text: String,

    /// Genetic search parameters.
    pub search: Search,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
pub struct Search {
    pub max_generations: u64,
    pub population_size: usize,
    pub boltzmann_temperature: f64,
    pub mutation_rate: f64,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            vasp_job_dir: "template".into(),
            num_magnetic_atoms: 5,
            ini_magmom_value: 5.0,
            placeholder_text: "XXXXX".into(),
            search: Search {
                population_size: 10,
                max_generations: 10,
                mutation_rate: 0.1,
                boltzmann_temperature: 30000.0,
            },
        }
    }
}

impl Config {
    pub fn print_toml(&self) {
        let x = toml::to_string(self).unwrap();
        println!("{:}", x);
    }
}
// config.rs:1 ends here
