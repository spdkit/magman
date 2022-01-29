// [[file:../magman.note::d4bc87e0][d4bc87e0]]
use std::path::PathBuf;

use gut::cli::*;
use gut::prelude::*;

/// Predict ground-state magnetic ordering of magnetic system.
#[derive(Debug, StructOpt)]
struct Cli {
    #[structopt(flatten)]
    verbose: Verbosity,

    /// Prints default configuration.
    #[structopt(long = "print", short = 'p')]
    print: bool,

    /// List calculated items in database.
    #[structopt(long = "list", short = 'l')]
    list: bool,

    /// Collect data from completed job files.
    #[structopt(long = "collect", short = 'c', parse(from_os_str))]
    collect: Option<PathBuf>,

    /// Run genetic search.
    #[structopt(long = "run", short = 'r')]
    run: bool,

    /// Specifies the number of jobs to run simultaneously
    #[structopt(long = "jobs", short = 'j', default_value = "1")]
    njobs: usize,
}

fn main() -> Result<()> {
    let args = Cli::from_args();
    args.verbose.setup_logger();

    if args.print {
        println!("{:#^72}", " default configuration ");
        magman::Config::default().print_toml();
        return Ok(());
    }

    // run in serial by default
    let njobs = args.njobs;
    std::env::set_var("RAYON_NUM_THREADS", njobs.to_string());
    if njobs > 1 {
        println!("Run {njobs} in parallel");
    }

    if args.run {
        if let Err(err) = magman::genetic_search() {
            bail!("genetic search failure: {err:?}");
        }
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
// d4bc87e0 ends here
