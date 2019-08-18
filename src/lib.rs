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
mod magmom;

pub use config::*;
pub use search::*;

pub(crate) mod common {
    pub use quicli::prelude::*;
    pub type Result<T> = ::std::result::Result<T, Error>;
}
// mods:1 ends here

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
