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

pub fn genetic_search() {
    let config = &crate::config::MAGMAN_CONFIG.search;

    let initial_population = create_population(config.population_size);
    println!("{:?}", initial_population);

    let algorithm = genetic_algorithm()
        .with_evaluation(MagFitnessEvaluator)
        .with_selection(RouletteWheelSelector::new(0.7, 2))
        .with_crossover(MultiPointCrossBreeder::new(3))
        .with_mutation(RandomValueMutator::new(config.mutation_rate, false, true))
        .with_reinsertion(ElitistReinserter::new(MagFitnessEvaluator, false, 0.7))
        .with_initial_population(initial_population)
        .build();

    let mut magcalc_sim = simulate(algorithm)
        .until(GenerationLimit::new(config.max_generations))
        .build();

    loop {
        let result = magcalc_sim.step();
        match result {
            Ok(SimResult::Intermediate(step)) => {
                let evaluated_population = step.result.evaluated_population;
                let best_solution = step.result.best_solution;
                println!(
                    "Step: generation: {}, average_fitness: {}, best fitness: {}",
                    step.iteration,
                    evaluated_population.average_fitness(),
                    best_solution.solution.fitness,
                );
                println!("{:?}", best_solution);
            }
            Ok(SimResult::Final(step, processing_time, duration, stop_reason)) => {
                let best_solution = step.result.best_solution;
                println!("{}", stop_reason);
                println!(
                    "Final result after {}: generation: {},
                     best solution with fitness {} found in generation {}, processing_time: {}",
                    duration,
                    step.iteration,
                    best_solution.solution.fitness,
                    best_solution.generation,
                    processing_time
                );
                break;
            }
            Err(e) => {
                println!("{}", e);
                break;
            }
        }
    }
}
// genevo:1 ends here

// fitness

// [[file:~/Workspace/Programming/structure-predication/magman/magman.note::*fitness][fitness:1]]
#[derive(Clone, Debug, PartialEq)]
struct MagFitnessEvaluator;

impl FitnessFunction<MagGenome, u32> for MagFitnessEvaluator {
    fn fitness_of(&self, individual: &MagGenome) -> u32 {
        use crate::magmom::*;

        // with the first bit fixed as spin-up.
        let mut so = individual.clone();
        so.insert(0, true);

        // NOTE: use data in csv table for tests
        // let csv = crate::vasp::CsvEvaluator {};
        // let ms = csv.evaluate(&so).expect("indv eval");

        let vasp = &crate::config::MAGMAN_CONFIG.vasp;
        let ms = vasp.evaluate(&so).expect("inv eval");
        calc_fitness(ms.energy)
    }

    fn average(&self, fitness_values: &[u32]) -> u32 {
        (fitness_values.iter().sum::<u32>() / fitness_values.len() as u32)
    }

    fn highest_possible_fitness(&self) -> u32 {
        1000
    }

    fn lowest_possible_fitness(&self) -> u32 {
        0
    }
}

fn calc_fitness(energy: f64) -> u32 {
    let temperature = 8000.;
    let eref = -205.40;
    let value = (energy - eref) * 96.;
    let fitness = (-1.0 * value / (temperature * 0.0083145)).exp();

    (fitness * 1000.) as u32
}

#[test]
#[ignore]
fn test_calc_fit() {
    let x = -205.30249;
    let x = calc_fitness(x);
    println!("{:#?}", x);
}
// fitness:1 ends here
