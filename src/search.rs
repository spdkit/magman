// [[file:../magman.note::fadfe03d][fadfe03d]]
use super::*;
use std::path::Path;

use spdkit::encoding::Binary;
use spdkit::population::Population;
use spdkit::prelude::*;
// fadfe03d ends here

// [[file:../magman.note::c0ca7449][c0ca7449]]
type MagGenome = Binary;

#[derive(Debug, Clone)]
struct MagIndividual;

impl EvaluateObjectiveValue<MagGenome> for MagIndividual {
    fn evaluate(&self, genome: &MagGenome) -> f64 {
        let key = genome.to_string();
        evaluate_magmom(genome).unwrap_or_else(|e| panic!("evaluation failed with error: {:?}", e))
    }
}

fn evaluate_magmom(indv: &MagGenome) -> Result<f64> {
    use crate::magmom::*;

    // with the first bit fixed as spin-up.
    let mut so = indv.clone();
    so.insert(0, true);

    // NOTE: use data in csv table for tests
    // let csv = crate::vasp::CsvEvaluator {};
    // let ms = csv.evaluate(&so).expect("indv eval");

    let vasp = &crate::config::MAGMAN_CONFIG.vasp;
    let ms = vasp.evaluate(&so).context("vasp evaluation")?;
    match EVALUATED.lock() {
        Ok(mut map) => {
            let key = so.to_string();
            map.insert(key, ms.energy);
            Ok(ms.energy)
        }
        Err(err) => {
            bail!("lock map failed: {:?}", err)
        }
    }
}
// c0ca7449 ends here

// [[file:../magman.note::809ad587][809ad587]]
use std::path::PathBuf;

pub struct StopHandler {
    stop_file: PathBuf,
}

impl StopHandler {
    pub fn new() -> Self {
        let stop_file = PathBuf::from("STOP");
        if stop_file.exists() {
            debug!("removing existing STOP file ...");
            let _ = std::fs::remove_file(&stop_file);
        }
        Self {
            stop_file: PathBuf::from("STOP"),
        }
    }

    fn is_interrupted(&self) -> bool {
        self.stop_file.exists()
    }

    /// Return error if finding a STOP file.
    pub fn handle_user_interruption(&self) -> Result<()> {
        if self.is_interrupted() {
            bail!("found stop file, stopping now ...");
        } else {
            Ok(())
        }
    }
}
// 809ad587 ends here

// [[file:../magman.note::2bff375c][2bff375c]]
use std::collections::HashMap;
use std::sync::Mutex;

use gosh::runner::stop::StopFileHandler;
use spdkit::operators::selection::StochasticUniversalSampling as SusSelection;
use spdkit::operators::selection::TournamentSelection;
use spdkit::operators::variation::TriadicCrossOver;

lazy_static! {
    static ref EVALUATED: Mutex<HashMap<String, f64>> = Mutex::new(HashMap::new());
}

pub fn genetic_search() -> Result<()> {
    let config = &crate::config::MAGMAN_CONFIG.search;

    // FIXME: genome length
    let length = config.genome_length - 1;

    // create a valuer gear
    let temperature = config.boltzmann_temperature;
    let valuer = spdkit::Valuer::new()
        .with_fitness(spdkit::fitness::MinimizeEnergy::new(temperature))
        .with_creator(MagIndividual);

    // create a breeder for new individuals
    let breeder = spdkit::GeneticBreeder::new()
        .with_crossover(TriadicCrossOver)
        .with_selector(SusSelection::new(3));

    // setup the algorithm
    let algo = spdkit::EvolutionAlgorithm::new(breeder, spdkit::Survivor::create().remove_duplicates(true));
    let stop = StopFileHandler::new();
    // FIXMEFIXMEFIXME
    let seeds = build_initial_genomes(config.population_size, length);
    for g in spdkit::Engine::create()
        .valuer(valuer)
        .algorithm(algo)
        .termination_nlast(config.termination_nlast)
        .evolve(&seeds)
        .take(config.max_generations)
    {
        let generation = g?;
        generation.summary();
        let energy = generation.population.best_member().unwrap().objective_value();

        if let Some(target_energy) = config.target_energy {
            if energy < target_energy {
                println!("target energy {} reached.", target_energy);
                break;
            }
        }
        stop.handle_user_interruption()?;

        // population convergence
        // let members: Vec<_> = generation.population.members().collect();
        // let mut pop_diversity = 0;
        // for p in members.windows(2) {
        //     let (g0, g1) = (p[0].individual.genome(), p[1].individual.genome());

        //     // sum over individual hamming distance.
        //     let dsum = g0
        //         .iter()
        //         .zip(g1.iter())
        //         .fold(0, |acc, (b0, b1)| acc + ((b0 != b1) as isize));
        //     pop_diversity += dsum;
        // }
        // info!("population diversity degree = {}", pop_diversity);
        // if pop_diversity == 0 {
        //     println!("population converged.");
        //     break;
        // }
    }

    let map = EVALUATED.lock().unwrap();
    println!("Explored {} combinations.", map.len());

    Ok(())
}

fn build_initial_genomes(n: usize, m: usize) -> Vec<Binary> {
    info!("Initialize {} genomes (genome size: {})", n, m);

    (0..n).map(|_| random_binary(m)).collect()
}

fn random_binary(length: usize) -> Binary {
    let mut rng = spdkit::get_rng!();
    let list: Vec<_> = (0..length).map(|_| rng.gen()).collect();
    Binary::new(list)
}
// 2bff375c ends here
