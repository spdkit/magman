// config.rs
// :PROPERTIES:
// :header-args: :tangle src/config.rs
// :END:

// [[file:~/Workspace/Programming/structure-predication/magman/magman.note::*config.rs][config.rs:1]]
use serde::*;
use toml;

#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
pub struct Config {
    pub runfile_sp: String,
    pub runfile_opt: String,
    pub molfile: String,
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
            runfile_sp: "/share/apps/mopac/sp".into(),
            runfile_opt: "/share/apps/mopac/opt".into(),
            molfile: "test.mol2".into(),
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
