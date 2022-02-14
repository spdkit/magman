// [[file:../magman.note::3a532d42][3a532d42]]
use super::*;

use gut::cli::*;
use gut::fs::*;
// 3a532d42 ends here

// [[file:../magman.note::ae28adb7][ae28adb7]]
/// Predict ground-state magnetic ordering of magnetic system.
#[derive(Debug, StructOpt)]
struct MagorderCli {
    #[structopt(flatten)]
    verbose: Verbosity,

    /// The path to VASP OUTCAR
    #[structopt()]
    outcar: PathBuf,
}

pub fn magorder_enter_main() -> Result<()> {
    let args = MagorderCli::from_args();
    args.verbose.setup_logger();

    crate::magorder::validate_magnetization(&args.outcar)?;

    Ok(())
}
// ae28adb7 ends here
