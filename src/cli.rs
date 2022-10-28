use clap::{command, arg, Parser};
use std::{path::{PathBuf, Path}, process::ExitStatus};

use crate::{RunError, run_round};



#[derive(Parser)]
#[command(author, version, about)]
pub struct Cli {
    /// the test-case generator program
    #[arg(short, long, value_name = "FILE")]
    pub generator: PathBuf,

    /// the program to be examined
    #[arg(short, long, value_name = "FILE")]
    pub program: PathBuf,

    /// the reference program/s
    #[arg(short, long, alias = "ref", action = clap::ArgAction::Append, required = true)]
    pub reference: Vec<PathBuf>,

    /// for how many rounds should the program be ran
    #[arg(short = 'c', long)]
    pub rounds: Option<u64>,
}



fn verify_output(inp: String, outs: Vec<(&Path, String)>) -> bool {
    let program = outs[0].clone();
    let references = outs.into_iter().skip(1);
    let bad: Vec<_> = references
        .filter(|(_, x)| x != &program.1)
        .collect();

    if bad.is_empty() {
        cli_section("Awesome! All references match the output!", true);
        true
    }else{
        cli_section(format!("there are {} mismatched testcases!", bad.len()).as_str(), false);

        println!("\n::: input:");
        println!("{}", inp);

        println!("\n::: program ({}) output:", program.0.display());
        println!("{}", program.1);
            
        for (p, out) in bad {
            println!("\n::: reference program ({}) output:", p.display());
            println!("{}", out);            
        }
        false
    }
}

fn cli_section(s: &str, ok: bool) {
    println!("{} -- {}", if ok {"✔"} else {"❌"}, s)
}

fn display_stderr(inp: String, xs: Vec<(PathBuf, ExitStatus, String)>) {
    cli_section("Some programs failed", false);
    for (p, s, e) in xs {
        println!("  - program {} failed with status {} and the error: {}", p.display(), s, e);
    }
    println!("the input provided was: {}", inp);
}

pub fn handle_cli(args: Cli){
    for round in 0..args.rounds.unwrap_or(1) {
        println!("== starting round {}", round);

        let outs = run_round(&args);
        let mut quit = outs.is_err();
    
        match outs {
            Err(RunError::StdErrs(inp, xs)) => display_stderr(inp, xs),
            Ok((inp, outs)) => quit = quit || !verify_output(inp, outs),
        }

        if quit { break; }
    }
}