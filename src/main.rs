// main

// [[file:~/Workspace/Programming/structure-predication/magman/magman.note::*main][main:1]]
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

    /// Path to configuration file of magman.
    #[structopt(parse(from_os_str), short = "c")]
    configfile: Option<std::path::PathBuf>,
}

fn main() -> CliResult {
    let args = Cli::from_args();
    args.verbosity.setup_env_logger(&env!("CARGO_PKG_NAME"))?;

    if args.print {
        println!("{:#^72}", " default configuration ");
        magman::Config::default().print_toml();
        return Ok(());
    }

    if let Some(f) = &args.configfile {
        println!("use configfile {}", f.display());
        let toml_str = read_file(f)?;
        let config: magman::Config = toml::from_str(&toml_str)?;

        magman::search();
    }

    Ok(())
}
// main:1 ends here
