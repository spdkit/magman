// header

// [[file:~/Workspace/Programming/structure-predication/magman/magman.note::*header][header:1]]
//! Predict ground-state magnetic ordering of magnetic system
// header:1 ends here

// imports

// [[file:~/Workspace/Programming/structure-predication/magman/magman.note::*imports][imports:1]]
#[macro_use]
extern crate lazy_static;
// imports:1 ends here

// mods

// [[file:~/Workspace/Programming/structure-predication/magman/magman.note::*mods][mods:1]]
mod config;
mod search;

pub use config::Config;
pub use search::search;

pub(crate) mod common {
    pub use quicli::prelude::*;
    pub type Result<T> = ::std::result::Result<T, Error>;
}
// mods:1 ends here

// energy

// [[file:~/Workspace/Programming/structure-predication/magman/magman.note::*energy][energy:1]]
use crate::common::*;

pub type SpinOrdering = Vec<bool>;

pub trait EvaluateSpinConfig {
    /// Evaluate system energy with specific spin ordering.
    fn get_energy(so: &SpinOrdering) -> Result<f64>;
}
// energy:1 ends here

// magmom

// [[file:~/Workspace/Programming/structure-predication/magman/magman.note::*magmom][magmom:1]]
pub struct Magmom {
    spin_ordering: SpinOrdering,
}

impl Magmom {
    // Initial magnetic moment value without considering of spin ordering.
    fn format_as_vasp_tag(&self, ini_magmom_value: f64) -> String {
        let ss: Vec<_> = self
            .spin_ordering
            .iter()
            .map(|&spin_up| {
                let v = if spin_up { 1.0 } else { -1.0 } * ini_magmom_value;
                format!("{:4.1}", v)
            })
            .collect();
        ss.join(" ")
    }
}

#[test]
fn test_vasp_tag() {
    let mm = Magmom {
        spin_ordering: vec![true, true, false, false],
    };

    let s = mm.format_as_vasp_tag(5.0);
    assert_eq!(s, " 5.0  5.0 -5.0 -5.0");
}

impl EvaluateSpinConfig for Magmom {
    fn get_energy(so: &SpinOrdering) -> Result<f64> {
        unimplemented!()
    }
}
// magmom:1 ends here
