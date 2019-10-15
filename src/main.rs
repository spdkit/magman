// main.rs
// :PROPERTIES:
// :header-args: :tangle src/main.rs
// :END:

// [[file:~/Workspace/Programming/structure-predication/magman/magman.note::*main.rs][main.rs:1]]
use std::path::PathBuf;

use quicli::prelude::*;
use structopt::*;

/// Predict ground-state magnetic ordering of magnetic system.
#[derive(Debug, StructOpt)]
struct Cli {
    #[structopt(flatten)]
    verbosity: Verbosity,

    /// Prints default configuration.
    #[structopt(long = "print", short = "p")]
    print: bool,

    /// List calculated items in database.
    #[structopt(long = "list", short = "l")]
    list: bool,

    /// Collect data from completed job files.
    #[structopt(long = "collect", short = "c", parse(from_os_str))]
    collect: Option<PathBuf>,

    /// Run genetic search.
    #[structopt(long = "run", short = "r")]
    run: bool,
}

fn main() -> CliResult {
    let args = Cli::from_args();
    args.verbosity.setup_env_logger(&env!("CARGO_PKG_NAME"))?;

    if args.print {
        println!("{:#^72}", " default configuration ");
        magman::Config::default().print_toml();
        return Ok(());
    }

    if args.run {
        magman::genetic_search();
    } else if args.list {
        // setup a pager like `less` cmd
        pager::Pager::with_pager("less").setup();
        magman::list_db()?;
    } else if let Some(dir) = args.collect {
        magman::collect_results_from_dir(&dir)?;
    } else {
        Cli::clap().print_help()?;
    }

    Ok(())
}
// main.rs:1 ends here
