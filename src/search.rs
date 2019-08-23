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
use std::collections::HashMap;
use std::sync::Mutex;

lazy_static! {
    static ref EVALUATED: Mutex<HashMap<String, f64>> = Mutex::new(HashMap::new());
}

type MagGenome = Vec<bool>;

/// TODO: with contrain of zero net-spin
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
    let mut map = EVALUATED.lock().unwrap();
    let key = crate::magmom::binary_key(&so);
    map.insert(key, ms.energy);

    Ok(ms.energy)
}

// summary population of each step
macro_rules! summary_results {
    ($step:expr) => {{
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

        energy
    }};
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

    let feval1 = MagFitnessEvaluator::new(eref);
    let feval2 = MagFitnessEvaluator::new(eref);
    let algorithm = genetic_algorithm()
        .with_evaluation(feval1)
        .with_selection(RouletteWheelSelector::new(0.9, 3))
        // .with_selection(MagSelector::new(0.9, 3))
        // .with_crossover(MultiPointCrossBreeder::new(3))
        .with_crossover(MagCrossover::new())
        .with_mutation(RandomValueMutator::new(config.mutation_rate, false, true))
        .with_reinsertion(MagReinsert::new(feval2))
        .with_initial_population(initial_population)
        .build();

    let mut magman_sim = simulate(algorithm)
        .until(GenerationLimit::new(config.max_generations))
        .build();

    loop {
        let result = magman_sim.step();
        match result {
            Ok(SimResult::Intermediate(step)) => {
                let energy = summary_results!(step);

                if let Some(target_energy) = config.target_energy {
                    if energy < target_energy {
                        println!("target energy {} reached.", target_energy);
                        break;
                    }
                }
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

    let map = EVALUATED.lock().unwrap();
    println!("Explored {} combinations.", map.len());

    Ok(())
}
// genevo:1 ends here

// reinsert

// [[file:~/Workspace/Programming/structure-predication/magman/magman.note::*reinsert][reinsert:1]]
use genevo::algorithm::EvaluatedPopulation;
use genevo::genetic::Offspring;
use genevo::operator::GeneticOperator;
use genevo::operator::ReinsertionOp;
use genevo::prelude::*;

#[derive(Clone, Debug, PartialEq)]
struct MagReinsert {
    feval: MagFitnessEvaluator,
}

impl MagReinsert {
    fn new(feval: MagFitnessEvaluator) -> Self {
        Self { feval }
    }
}

impl ReinsertionOp<MagGenome, u32> for MagReinsert {
    fn combine<R: Rng + Sized>(
        &self,
        new_individuals: &mut Offspring<MagGenome>,
        old_population: &EvaluatedPopulation<MagGenome, u32>,
        rng: &mut R,
    ) -> Vec<MagGenome> {
        // combine all available individuals into one.
        let old_individuals = old_population.individuals();
        new_individuals.extend_from_slice(&old_individuals);
        info!("{} indvs before combining ...", new_individuals.len());

        // remove redundant individuals.
        new_individuals.sort();
        new_individuals.dedup();
        info!("{} indvs after removing redundants.", new_individuals.len());

        new_individuals.sort_by_cached_key(|indv| self.feval.fitness_of(&indv));

        // remove n bad performing indvs to fit the population size constrain.
        let n = new_individuals.len() - old_individuals.len();

        debug!("Will remove {} indvs.", n);
        new_individuals.drain(n..).collect()
    }
}

impl GeneticOperator for MagReinsert {
    fn name() -> String {
        "Magman-Reinserter".to_string()
    }
}
// reinsert:1 ends here

// crossover

// [[file:~/Workspace/Programming/structure-predication/magman/magman.note::*crossover][crossover:1]]
use genevo::operator::CrossoverOp;

#[derive(Clone, Debug, PartialEq)]
struct MagCrossover {
    //
}

impl GeneticOperator for MagCrossover {
    fn name() -> String {
        "Magman-Crossover".to_string()
    }
}

impl MagCrossover {
    fn new() -> Self {
        Self {
            // -
        }
    }
}

impl CrossoverOp<MagGenome> for MagCrossover {
    fn crossover<R>(&self, parents: Vec<MagGenome>, rng: &mut R) -> Vec<MagGenome>
    where
        R: Rng + Sized,
    {
        debug!("breed new individuals using {} parents.", parents.len());
        let mut evaluated_indvs: Vec<_> = parents
            .iter()
            .map(|indv| {
                let energy = evaluate_individual(&indv).expect("x");
                (indv, energy)
            })
            .collect();

        // sort by energy from lowest to highest
        evaluated_indvs
            .sort_by(|(_, a), (_, b)| a.partial_cmp(&b).unwrap_or(std::cmp::Ordering::Less));

        for (indv, energy) in evaluated_indvs.iter() {
            let key = crate::magmom::binary_key(&indv);
            debug!(">> {} = {}", key, energy);
        }

        let (parent0, _) = &evaluated_indvs[0];
        let (parent1, _) = &evaluated_indvs[1];
        let (parent2, _) = &evaluated_indvs[2];

        let positions_swap: Vec<_> = parent0
            .iter()
            .zip(parent1.iter())
            .enumerate()
            .filter_map(|(i, (so1, so2))| if so1 == so2 { Some(i) } else { None })
            .collect();

        let mut child1 = parent1.to_vec();
        let mut child2 = parent2.to_vec();
        for i in positions_swap {
            std::mem::swap(&mut child1[i], &mut child2[i]);
        }

        let children = vec![child1, child2];
        for indv in children.iter() {
            let key = crate::magmom::binary_key(&indv);
            let energy = evaluate_individual(&indv).expect("x");
            debug!("new child: {} = {}", key, energy);
        }

        // let breeder = MultiPointCrossBreeder::new(3);
        // breeder.crossover(parents[0..2].to_vec(), rng)

        children
    }
}
// crossover:1 ends here

// fitness

// [[file:~/Workspace/Programming/structure-predication/magman/magman.note::*fitness][fitness:1]]
#[derive(Clone, Debug, PartialEq)]
struct MagFitnessEvaluator {
    eref: f64,
}

impl MagFitnessEvaluator {
    fn new(eref: f64) -> Self {
        Self { eref }
    }
}

const FITNESS_MAX: u32 = 1_000_000;

impl FitnessFunction<MagGenome, u32> for MagFitnessEvaluator {
    fn fitness_of(&self, individual: &MagGenome) -> u32 {
        let key = crate::magmom::binary_key(individual);
        let energy = evaluate_individual(&individual).expect("inv eval");
        calc_fitness(energy, self.eref)
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
