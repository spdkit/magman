// imports

// [[file:~/Workspace/Programming/structure-predication/magman/magman.note::*imports][imports:1]]
use std::path::Path;

use crate::common::*;

use genevo::operator::prelude::*;
use genevo::population::{BinaryEncodedGenomeBuilder, ValueEncodedGenomeBuilder};
use genevo::prelude::*;
// imports:1 ends here

// genevo

// [[file:~/Workspace/Programming/structure-predication/magman/magman.note::*genevo][genevo:1]]
type MagGenome = Vec<bool>;

fn create_population(n: usize) -> Population<MagGenome> {
    build_population()
        .with_genome_builder(BinaryEncodedGenomeBuilder::new(11))
        .of_size(n)
        .uniform_at_random()
}

fn evaluate_individual(indv: &MagGenome) -> Result<f64> {
    use crate::magmom::*;

    // with the first bit fixed as spin-up.
    let mut so = indv.clone();
    so.insert(0, true);

    // NOTE: use data in csv table for tests
    // let csv = crate::vasp::CsvEvaluator {};
    // let ms = csv.evaluate(&so).expect("indv eval");

    let vasp = &crate::config::MAGMAN_CONFIG.vasp;
    let ms = vasp.evaluate(&so)?;

    Ok(ms.energy)
}

// summary population of each step
macro_rules! summary_results {
    ($step:ident) => {
        let evaluated_population = $step.result.evaluated_population;
        let best_solution = $step.result.best_solution;
        let energy = evaluate_individual(&best_solution.solution.genome).expect("evaluated energy");
        println!(
            "Generation: {:^5} energy of current best = {:<-20.4}",
            $step.iteration, energy
        );

        for indv in evaluated_population.individuals().iter() {
            let key = crate::magmom::binary_key(&indv);
            let energy = evaluate_individual(&indv).expect("evaluated indv");
            println!("{} => {}", key, energy);
        }
        debug!(
            "average_fitness: {}",
            evaluated_population.average_fitness()
        );
        debug!("best fitness: {}", best_solution.solution.fitness);
    };
}

pub fn genetic_search() -> Result<()> {
    let config = &crate::config::MAGMAN_CONFIG.search;

    let initial_population = create_population(config.population_size);
    info!(
        "Evaluting initial population ({} individuals)",
        initial_population.size()
    );

    // set reference energy for fitness as the lowest energy individual in the
    // population.
    let mut eref = std::f64::MAX;
    for indv in initial_population.individuals() {
        let energy = evaluate_individual(indv)?;
        if energy < eref {
            eref = energy;
        }
    }
    info!("Reference energy for fitness evaluation is: {}", eref);

    let algorithm = genetic_algorithm()
        .with_evaluation(MagFitnessEvaluator(eref))
        .with_selection(RouletteWheelSelector::new(0.7, 2))
        .with_crossover(MultiPointCrossBreeder::new(3))
        .with_mutation(RandomValueMutator::new(config.mutation_rate, false, true))
        .with_reinsertion(ElitistReinserter::new(
            MagFitnessEvaluator(eref),
            false,
            0.7,
        ))
        .with_initial_population(initial_population)
        .build();

    let mut magman_sim = simulate(algorithm)
        .until(GenerationLimit::new(config.max_generations))
        .build();

    loop {
        let result = magman_sim.step();
        match result {
            Ok(SimResult::Intermediate(step)) => {
                summary_results!(step);
            }
            Ok(SimResult::Final(step, _processing_time, _duration, stop_reason)) => {
                summary_results!(step);

                println!("{}", stop_reason);
                break;
            }
            Err(e) => {
                println!("{}", e);
                break;
            }
        }
    }

    Ok(())
}
// genevo:1 ends here

// fitness

// [[file:~/Workspace/Programming/structure-predication/magman/magman.note::*fitness][fitness:1]]
#[derive(Clone, Debug, PartialEq)]
struct MagFitnessEvaluator(f64);

const FITNESS_MAX: u32 = 1_000_000;

impl FitnessFunction<MagGenome, u32> for MagFitnessEvaluator {
    fn fitness_of(&self, individual: &MagGenome) -> u32 {
        let energy = evaluate_individual(&individual).expect("inv eval");
        debug!(
            "energy of spin-ordering {} = {}",
            crate::magmom::binary_key(&individual),
            energy
        );

        let eref = self.0;
        calc_fitness(energy, eref)
    }

    fn average(&self, fitness_values: &[u32]) -> u32 {
        (fitness_values.iter().sum::<u32>() / fitness_values.len() as u32)
    }

    fn highest_possible_fitness(&self) -> u32 {
        FITNESS_MAX
    }

    fn lowest_possible_fitness(&self) -> u32 {
        0
    }
}

fn calc_fitness(energy: f64, eref: f64) -> u32 {
    let config = &crate::config::MAGMAN_CONFIG.search;
    let temperature = config.boltzmann_temperature;
    let value = (energy - eref) * 96.;
    let fitness = (-1.0 * value / (temperature * 0.0083145)).exp();

    (fitness * FITNESS_MAX as f64) as u32
}
// fitness:1 ends here
