// core

// [[file:~/Workspace/Programming/structure-predication/magman/magman.note::*core][core:1]]
use serde::*;

use crate::common::*;
use crate::MAG_DB_CONNECTION;

use gosh_db::prelude::*;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MagneticState {
    pub spin_ordering: Vec<bool>,
    pub energy: f64,
}

pub trait EvaluateMagneticState {
    /// Evaluate with caching.
    fn evaluate(&self, so: &[bool]) -> Result<MagneticState> {
        let key = binary_key(so);
        match crate::magmom::MagneticState::get_from_collection(&MAG_DB_CONNECTION, &key) {
            Ok(ms) => Ok(ms),
            // FIXME: handle not-found error
            Err(e) => {
                let ms = self.evaluate_new(so)?;
                ms.put_into_collection(&MAG_DB_CONNECTION, &key)?;
                Ok(ms)
            }
        }
    }

    /// Evaluate new item.
    fn evaluate_new(&self, so: &[bool]) -> Result<MagneticState>;
}

impl Collection for MagneticState {
    fn collection_name() -> String {
        "MAGMOM".into()
    }
}

impl MagneticState {
    pub fn new(so: &[bool], energy: f64) -> Self {
        Self {
            spin_ordering: so.to_owned(),
            energy,
        }
    }
}

/// Return binary encoded key of a spin-ordering.
pub fn binary_key(so: &[bool]) -> String {
    let ss: String = so
        .iter()
        .map(|&spin_up| if spin_up { "1" } else { "0" })
        .collect();
    ss
}
// core:1 ends here
