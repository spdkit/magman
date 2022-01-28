// [[file:../magman.note::ea64b277][ea64b277]]
use gchemol_parser::GrepReader;
use gut::prelude::*;
use std::path::Path;

#[test]
#[ignore]
fn test_magorder() -> Result<()> {
    let f = "./tests/files/jobs/100100001001/OUTCAR";
    let f = "/home/ybyygu/Workspace/Projects/structure-prediction/磁态优化/data/d8/a212ae-a1ee-4691-b604-a4623f10f400/1-20/100011010100/OUTCAR";

    validate_magnetization(f.as_ref())?;

    Ok(())
}

pub fn validate_magnetization(f: &Path) -> Result<()> {
    let f = f.canonicalize().context("get full path")?;
    let mut reader = GrepReader::try_from_path(&f)?;
    let n = reader.mark(&[r"^ magnetization \(x\)$"])?;
    assert!(n > 0, "no magnetization output found! (Tip: set LORBIT=11 in INCAR)");

    for _ in 0..n {
        let _ = reader.goto_next_marker()?;
    }

    // let input_orders = parse_mag_order_from_path(f.as_ref()).ok_or_else(|| format_err!("invalid f: {f:?}"))?;
    // let nmag_bit = input_orders.len();
    // debug!("number of magnetic atoms: {:}", nmag_bit);

    let s = gut::fs::read_file(f.with_file_name("INCAR"))?;
    let input_orders = parse_mag_order_from_incar(&s);
    let nmag_bit = input_orders.len();
    info!("number of magnetic atoms: {:}", nmag_bit);
    assert!(nmag_bit > 1, "invalid number of magnetic atoms: {}", nmag_bit);

    let mut s = String::new();
    let nlines = nmag_bit + 4;
    let _ = reader.read_lines(nlines, &mut s)?;
    for (line, bit) in s.lines().skip(4).zip(input_orders) {
        info!("{}", line);
        if let Some(mag) = line.split_whitespace().last().and_then(|x| x.parse::<f64>().ok()) {
            if mag.is_sign_positive() {
                if bit < 0.01 {
                    eprintln!("{mag} != {bit}");
                }
            } else if bit > 0.01 {
                eprintln!("{mag} != {bit}");
            }
        }
    }

    Ok(())
}

fn parse_mag_order_from_path(f: &Path) -> Option<&str> {
    let dir = f.parent()?.file_name()?;
    dir.to_str()
}

fn parse_mag_order_from_incar(s: &str) -> Vec<f64> {
    let mut parts: Vec<f64> = vec![];
    for line in s.lines() {
        let line = line.to_uppercase();
        //   MAGMOM =   5.0 -5.0 -5.0 -5.0  5.0  5.0 -5.0  5.0 -5.0  5.0 -5.0 -5.0 16*0.0
        if line.contains("MAGMOM") {
            parts = line.split_whitespace().skip(2).filter_map(|x| x.parse().ok()).collect();
        }
    }
    parts
}

#[test]
fn test_mag_order_from_incar() -> Result<()> {
    let f = "./tests/files/jobs/100100001001/INCAR";
    let s = gut::fs::read_file(f)?;
    let o = parse_mag_order_from_incar(&s);
    dbg!(o);
    Ok(())
}
// ea64b277 ends here
