// imports

// [[file:~/Workspace/Programming/structure-predication/magman/magman.note::*imports][imports:1]]
use std::path::Path;

use crate::common::*;

use spdkit::encoding::Binary;
use spdkit::prelude::*;
use spdkit::population::Population;
// imports:1 ends here

// individual

// [[file:~/Workspace/Programming/structure-predication/magman/magman.note::*individual][individual:1]]
type MagGenome = Binary;

#[derive(Debug, Clone)]
struct MagIndividual;

impl EvaluateScore<MagGenome> for MagIndividual {
    fn evaluate(&self, genome: &MagGenome) -> f64 {
        let key = genome.to_string();
        evaluate_magmom(genome).expect("inv eval")
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
    let ms = vasp.evaluate(&so)?;
    let mut map = EVALUATED.lock().unwrap();
    let key = so.to_string();
    map.insert(key, ms.energy);

    Ok(ms.energy)
}
// individual:1 ends here

// core

// [[file:~/Workspace/Programming/structure-predication/magman/magman.note::*core][core:1]]
use std::collections::HashMap;
use std::sync::Mutex;

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
    let initial_population = build_initial_population(config.population_size, length);

    // create a breeder for new individuals
    let breeder = spdkit::gears::breeder::GeneticBreeder::new()
        .with_crossover(TriadicCrossOver)
        // .with_selector(TournamentSelection::new(3));
        .with_selector(SusSelection::new(3));

    let temperature = config.boltzmann_temperature;
    let mut engine = Engine::new(initial_population)
        .with_creator(MagIndividual)
        .with_fitness(spdkit::fitness::MinimizeEnergy::new(temperature))
        .with_breeder(breeder);

    // FIXMEFIXMEFIXME
    let mut best_so_far = std::f64::MAX;
    let mut ichanges = 0;
    for g in engine.evolve().take(config.max_generations) {
        let generation = g?;
        generation.summary();
        let energy = generation
            .population
            .best_member()
            .unwrap()
            .individual
            .raw_score();

        if let Some(target_energy) = config.target_energy {
            if energy < target_energy {
                println!("target energy {} reached.", target_energy);
                break;
            }
        }
        if energy < best_so_far {
            ichanges = 0;
            best_so_far = energy;
        } else {
            ichanges += 1;
        }

        if ichanges > 20 {
            println!("population evolved for 20 generations without changes. stop now.");
            break;
        }

        // population convergence
        let members: Vec<_> = generation.population.members().collect();
        let mut pop_diversity = 0;
        for p in members.windows(2) {
            let (g0, g1) = (p[0].individual.genome(), p[1].individual.genome());

            // sum over individual hamming distance.
            let dsum = g0
                .iter()
                .zip(g1.iter())
                .fold(0, |acc, (b0, b1)| acc + ((b0 != b1) as isize));
            pop_diversity += dsum;
        }
        info!("population diversity degree = {}", pop_diversity);
        if pop_diversity == 0 {
            println!("population converged.");
            break;
        }
    }

    let map = EVALUATED.lock().unwrap();
    println!("Explored {} combinations.", map.len());

    Ok(())
}

fn build_initial_population(n: usize, m: usize) -> Population<Binary> {
    info!(
        "Build initial population ({} individuals, genome size: {})",
        n, m
    );

    let keys: Vec<_> = (0..n).map(|_| random_binary(m)).collect();

    let indvs = MagIndividual.create(keys);

    spdkit::population::Builder::new(spdkit::fitness::MinimizeEnergy::new(5000.0))
        .size_limit(n)
        .build(indvs)
}

fn random_binary(length: usize) -> Binary {
    let mut rng = spdkit::get_rng!();
    let list: Vec<_> = (0..length).map(|_| rng.gen()).collect();
    Binary::new(list)
}
// core:1 ends here
