use clap::{command, arg, Parser};
use std::path::PathBuf;

use crate::{run_round, Failure, test_mismatch, Success};



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
    #[arg(short, long, alias = "ref", action = clap::ArgAction::Append)]
    pub reference: Vec<PathBuf>,

    /// for how many rounds should the program be ran
    #[arg(short = 'c', long)]
    pub rounds: Option<u64>,

    /// the time limit (in seconds) for the programme execution (references are left alone)
    #[arg(short = 't', long)]
    pub time_limit: Option<f64>,

    /// the memory limit (in kilo-bytes) for the programme execution (references are left alone)
    #[arg(short = 'm', long)]
    pub memory_limit: Option<usize>,
}



fn display_mismatches(inp: String, prog: Success, refs: Vec<Success>) {
    cli_section(format!("there are {} mismatched testcases!", refs.len()).as_str(), false);

    println!("\n::: input:");
    println!("{}", inp);

    println!("\n::: program ({}) output:", prog.0.display());
    println!("{}", prog.1);
        
    for (p, out) in refs {
        println!("\n::: reference program ({}) output:", p.display());
        println!("{}", out);            
    }
}

fn display_ref_mismatches(inp: String, refs: Vec<Success>) {
    cli_section(format!("🚧 CRITICAL ERROR 🚧 there are {} mismatched references!!!!", refs.len()).as_str(), false);

    println!("\n::: input:");
    println!("{}", inp);
        
    for (p, out) in refs {
        println!("\n::: reference program ({}) output:", p.display());
        println!("{}", out);            
    }
}

fn cli_section(s: &str, ok: bool) {
    println!("{} -- {}", if ok {"✔"} else {"❌"}, s)
}

fn display_failure(fail: Failure) {
    match fail {
        Failure::Prog(path, status, err) => 
            println!("  👎 program \"{}\" failed with status \"{}\" and the error: {}", path.display(), status, err),
        Failure::TimeLimit(path) => 
            println!("  👎 program \"{}\" exceeded the time limit!", path.display()),
    }
}

pub fn handle_cli(args: Cli){
    for round in 0..args.rounds.unwrap_or(1) {
        println!("== starting round {}", round);

        let outs = run_round(&args);
        
        use crate::Round as R;
        use crate::Mismatch as M;
        match outs {
            R::GeneratorFail(fail) => display_failure(fail),
            R::ProgramFail(inp, fail) => {
                display_failure(fail);
                println!("with the following input: \n{}", inp);
            },
            R::ReferenceFails(_, fails) => fails.into_iter().for_each(display_failure),
            R::Success(inp, prog, refs) => if refs.is_empty() {
                println!("  🚧 warning : skipping reference checks as no references were supplied...")        
            } else { 
                match test_mismatch(prog, refs) {
                    M::AllMatch => cli_section("Awesome! All references match the output!", true),
                    M::ProgMismatch(prog, refs) => display_mismatches(inp, prog, refs),
                    M::RefMismatch(refs) => display_ref_mismatches(inp, refs),
                }
            }
        }
    }
}