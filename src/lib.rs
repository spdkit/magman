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
mod vasp;

pub use config::*;
pub use search::*;

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
    fn get_energy(&self, so: &SpinOrdering) -> Result<f64>;
}
// energy:1 ends here

// magmom

// [[file:~/Workspace/Programming/structure-predication/magman/magman.note::*magmom][magmom:1]]
use gosh_db::prelude::*;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MagneticState {
    spin_ordering: SpinOrdering,
    energy: Option<f64>,
}

impl Collection for MagneticState {
    fn collection_name() -> String {
        "MAG-MOM".into()
    }
}

impl MagneticState {
    pub fn new(so: &[bool]) -> Self {
        Self {
            spin_ordering: so.to_vec(),
            energy: None,
        }
    }

    // Initial magnetic moment value without considering of spin ordering.
    pub(crate) fn format_as_vasp_tag(&self, ini_magmom_value: f64) -> String {
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

    pub fn binary_key(&self) -> String {
        let ss: String = self
            .spin_ordering
            .iter()
            .map(|&spin_up| if spin_up { "1" } else { "0" })
            .collect();
        ss
    }
}

#[test]
fn test_vasp_tag() {
    let so = vec![true, true, false, false];
    let mm = MagneticState::new(&so);

    let s = mm.format_as_vasp_tag(5.0);
    assert_eq!(s, " 5.0  5.0 -5.0 -5.0");

    let k = mm.binary_key();
    assert_eq!(k, "1100");
}
// magmom:1 ends here

// global

// [[file:~/Workspace/Programming/structure-predication/magman/magman.note::*global][global:1]]
// global database connection
lazy_static! {
    static ref MAG_DB_CONNECTION: gosh_db::DbConnection = {
        let db = gosh_db::DbConnection::establish().expect("gosh db");
        db
    };
}
// global:1 ends here
