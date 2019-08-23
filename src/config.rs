// imports

// [[file:~/Workspace/Programming/structure-predication/magman/magman.note::*imports][imports:1]]
use serde::*;
use toml;

lazy_static! {
    /// Global settings.
    pub static ref MAGMAN_CONFIG: Config = {
        let config_file = format!("{}.conf", env!("CARGO_PKG_NAME"));
        println!("configfile {}", config_file);

        let toml_str = quicli::fs::read_file(config_file).expect("Failed to read config file!");
        toml::from_str(&toml_str).expect("Failed to parse toml config!")
    };
}
// imports:1 ends here

// base

// [[file:~/Workspace/Programming/structure-predication/magman/magman.note::*base][base:1]]
#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
pub struct Config {
    /// VASP related parameters.
    pub vasp: crate::vasp::Vasp,

    /// Genetic search parameters.
    pub search: Search,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
pub struct Search {
    pub max_generations: u64,
    pub target_energy: Option<f64>,
    pub population_size: usize,
    pub boltzmann_temperature: f64,
    pub mutation_rate: f64,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            vasp: crate::vasp::Vasp::default(),
            search: Search {
                population_size: 10,
                max_generations: 10,
                target_energy: None,
                mutation_rate: 0.1,
                boltzmann_temperature: 5000.0,
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
// base:1 ends here
