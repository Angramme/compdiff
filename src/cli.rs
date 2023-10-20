use clap::{command, arg, Parser};
use std::{path::PathBuf, env};

use crate::{run_round, Failure, test_mismatch, Success, preprocess_command};



#[derive(Parser)]
#[command(author, version, about)]
pub struct Cli {
    /// the test-case generator programme
    #[arg(short, long, value_name = "FILE")]
    pub generator: PathBuf,

    /// the programme to be examined
    #[arg(short, long, value_name = "FILE")]
    pub program: PathBuf,

    /// the reference programme/programmes
    #[arg(short, long, alias = "ref", action = clap::ArgAction::Append)]
    pub reference: Vec<PathBuf>,

    /// for how many rounds should the programme be ran
    #[arg(short = 'c', long)]
    pub rounds: Option<u64>,

    /// time limit (s) for the programme excluding references
    #[arg(short = 't', long)]
    pub time_limit: Option<f64>,

    /// memory limit (kB) for the programme excluding references
    #[arg(short = 'm', long)]
    pub memory_limit: Option<usize>,

    /// print additional information
    #[arg(short = 'v', long, default_value = "false")]
    pub verbose: bool,

    /// options for c++ compiler
    #[arg(long, default_value = "-std=c++20")]
    pub cpp_compiler_flags: String,
}



fn display_mismatches(inp: &String, prog: &Success, refs: &Vec<Success>) {
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

fn display_ref_mismatches(inp: &String, refs: &Vec<Success>) {
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

fn display_failure(fail: &Failure) {
    match fail {
        Failure::Prog(path, status, err) => 
            println!("  👎 program \"{}\" failed with status \"{}\" and the error: {}", path.display(), status, err),
        Failure::TimeLimit(path) => 
            println!("  👎 program \"{}\" exceeded the time limit!", path.display()),
    }
}

pub fn handle_cli(mut args: Cli){
    use crate::Round as R;
    use crate::Mismatch as M;

    if args.verbose {
        env::set_var("RUST_BACKTRACE", "1");
    }

    args.program = preprocess_command(args.program.clone(), &args).expect("failed preprocessing program!");
    args.generator = preprocess_command(args.generator.clone(), &args).expect("failed preprocessing generator!");
    args.reference = args.reference.iter().map(|s|
        preprocess_command(s, &args).expect("failed preprocessing reference!")
    ).collect();

    let mut fails = vec![];
    for round in 0..args.rounds.unwrap_or(1) {
        println!("== starting round {}", round);

        let outs = run_round(&args);
        
        match outs {
            R::GeneratorFail(fail) => display_failure(&fail),
            R::ProgramFail(inp, fail) => {
                display_failure(&fail);
                println!("with the following input: \n{}", inp);
            },
            R::ReferenceFails(inp, fails) => {
                fails.iter().for_each(display_failure);
                println!("with the following input: \n{}", inp);   
            },
            R::Success(inp, prog, refs) => if refs.is_empty() {
                println!("  🚧 warning : skipping reference checks as no references were supplied...")        
            } else { 
                if args.verbose { println!("running comparisons of output..."); }

                let test = test_mismatch(prog, refs);
                match test {
                    M::AllMatch => cli_section("Awesome! All references match the output!", true),
                    M::ProgMismatch(ref prog, ref refs) => display_mismatches(&inp, &prog, &refs),
                    M::RefMismatch(ref refs) => display_ref_mismatches(&inp, &refs),
                }
                if !matches!(test, M::AllMatch) {
                    fails.push((inp, test))
                }
            }
        }
    }

    if fails.is_empty() { return; }
    println!(" 🚧 Summary of all fails: ");

    for (inp, mismatch) in fails {
        match mismatch {
            M::ProgMismatch(prog, refs) => display_mismatches(&inp, &prog, &refs),
            M::RefMismatch(refs) => display_ref_mismatches(&inp, &refs),
            _ => panic!("internal error, unrecognized mismatch"),
        }
    }
}