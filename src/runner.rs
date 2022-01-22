// [[file:../magman.note::6074d240][6074d240]]
use super::*;
// 6074d240 ends here

// [[file:../magman.note::99a18400][99a18400]]
#[derive(Debug, Clone, PartialEq)]
enum NodeStatus {
    Ready,
    Busy,
    Down,
}

struct Node {
    name: String,
    status: NodeStatus,
}

impl Node {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.into(),
            status: NodeStatus::Ready,
        }
    }

    pub fn is_ready(&self) -> bool {
        self.status == NodeStatus::Ready
    }

    pub fn mark_as_ready(&mut self) {
        self.status = NodeStatus::Ready
    }

    pub fn mark_as_busy(&mut self) {
        self.status = NodeStatus::Busy
    }
}
// 99a18400 ends here

// [[file:../magman.note::*ssh][ssh:1]]
fn read_line_from_stdin() -> Result<String> {
    let mut buffer = String::new();
    stdin().read_line(&mut buffer).context("read from stdin")?;
    Ok(buffer)
}

fn remote_cmd_run_ssh(remote: &str, program: &str, wrk_dir: &str) -> Result<String> {
    use gut::cli::duct::cmd;

    let o = cmd!(
        "ssh",
        "-x",
        "-o",
        "StrictHostKeyChecking=no",
        remote,
        format!("cd {wrk_dir};"),
        program
    )
    .read()?;

    Ok(o)
}

#[test]
fn test_remote_ssh() -> Result<()> {
    let o = remote_cmd_run_ssh("localhost", "pwd", "/tmp")?;
    assert_eq!(o, "/tmp");

    Ok(())
}
// ssh:1 ends here

// [[file:../magman.note::f42b9239][f42b9239]]
use std::io::stdin;

impl Node {
    fn run_cmd(&mut self, program: &str, wrk_dir: &str) -> Result<()> {
        remote_cmd_run_ssh(&self.name, program, wrk_dir)?;

        Ok(())
    }

    /// Run programs in remote node, in a shell-like style
    pub fn run_interactively(&mut self) -> Result<()> {
        loop {
            println!("program to run:");
            let program = read_line_from_stdin()?;
            if program == "exit" || program.is_empty() {
                return Ok(());
            }
            println!("working directory:");
            let working_dir = read_line_from_stdin()?;
            self.run_cmd(&program, &working_dir)?;
        }
    }
}
// f42b9239 ends here

// [[file:../magman.note::01d80972][01d80972]]
use clap::Parser;

/// Run program in remote node interactively
#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Cli {
    /// The node name for running program.
    node: String,
}

pub fn remote_enter_main() -> Result<()> {
    let args = Cli::parse();
    Node::new(&args.node).run_interactively()?;

    Ok(())
}
// 01d80972 ends here
