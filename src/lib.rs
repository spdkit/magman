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
mod magmom;
mod search;
mod vasp;

pub use config::*;
pub use search::*;

pub(crate) mod common {
    pub use gut::prelude::*;
}
// mods:1 ends here

// global

// [[file:~/Workspace/Programming/structure-predication/magman/magman.note::*global][global:1]]
use crate::common::*;

// global database connection
lazy_static! {
    static ref MAG_DB_CONNECTION: gosh_db::DbConnection = {
        let dbvar = "GOSH_DATABASE_URL";
        let default_db = format!("{}.db", env!("CARGO_PKG_NAME"));
        if std::env::var(dbvar).is_err() {
            info!("Use default db file: {}", default_db);
            std::env::set_var(dbvar, default_db);
        }
        let db = gosh_db::DbConnection::establish().expect("gosh db");
        db
    };
}

pub fn list_db() -> Result<()> {
    magmom::MagneticState::list_db()?;

    Ok(())
}

/// Collect results from finished jobs in working directory.
pub fn collect_results_from_dir(d: &std::path::Path) -> Result<()> {
    let vasp = &crate::config::MAGMAN_CONFIG.vasp;
    debug!("collecting results ...");
    for ms in vasp.collect_results()? {
        ms.save().unwrap_or_else(|e| {
            error!("{}", e);
        });
    }
    debug!("db updated.");

    Ok(())
}
// global:1 ends here
