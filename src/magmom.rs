// [[file:../magman.note::c7167dd1][c7167dd1]]
use serde::*;

use super::*;
use crate::MAG_DB_CONNECTION;

use gosh::db::prelude::*;

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
            Err(_) => {
                let ms = self.evaluate_new(so).with_context(|| format!("evaluate {key}"))?;
                ms.put_into_collection(&MAG_DB_CONNECTION, &key)
                    .with_context(|| format!("put {key} into db"))?;
                Ok(ms)
            }
        }
    }

    /// Evaluate new item.
    fn evaluate_new(&self, so: &[bool]) -> Result<MagneticState>;
}

impl MagneticState {
    pub fn new(so: &[bool], energy: f64) -> Self {
        Self {
            spin_ordering: so.to_owned(),
            energy,
        }
    }

    /// Save into default database.
    pub fn save(&self) -> Result<()> {
        let key = binary_key(&self.spin_ordering);
        info!("saving data with key {}", key);
        self.put_into_collection(&MAG_DB_CONNECTION, &key)?;

        Ok(())
    }

    pub fn binary_key(&self) -> String {
        binary_key(&self.spin_ordering)
    }
}

/// Return binary encoded key of a spin-ordering.
pub fn binary_key(so: &[bool]) -> String {
    let ss: String = so.iter().map(|&spin_up| if spin_up { "1" } else { "0" }).collect();
    ss
}

impl MagneticState {
    pub fn list_db() -> Result<()> {
        let mut items = Self::list_collection(&MAG_DB_CONNECTION)?;
        if items.is_empty() {
            error!("No items in db.");
        } else {
            println!("Found {} items.", items.len());
            println!("{:^width$} => {:^12}", "key", "energy", width = items[0].spin_ordering.len());

            items.sort_by(|a, b| a.energy.partial_cmp(&b.energy).unwrap_or(std::cmp::Ordering::Less));
            for ms in items {
                let key = ms.binary_key();
                println!("{} => {:<-12.4}", key, ms.energy);
            }
        }
        Ok(())
    }
}
// c7167dd1 ends here

// [[file:../magman.note::*test][test:1]]
#[test]
fn test_list_db() -> Result<()> {
    MagneticState::list_db()?;

    Ok(())
}
// test:1 ends here
