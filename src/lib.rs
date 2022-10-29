pub mod cli;

use std::{process::{Command, Stdio, ExitStatus, Child}, io::Write, path::{Path, PathBuf}, env::current_dir};
use std::ffi::OsStr;
use std::error::Error;
use cli::Cli;
use string_error::{into_err, static_err};

fn get_command<P>(path: P) -> Result<Command, Box<dyn Error>>
where P: AsRef<Path>
{
    match path.as_ref().extension().and_then(OsStr::to_str) {
        Some("py") => get_python_command(path),
        Some("exe") | None => Ok(get_exe_command(path)),
        Some(x) => Err(into_err(format!("unsupported file type {}", x))),
    }
}

fn get_exe_command<P>(path: P) -> Command
where P: AsRef<Path>
{
    Command::new(path.as_ref())
}

fn get_python_command<P>(path: P) -> Result<Command, Box<dyn Error>>
where P: AsRef<Path>
{
    let pyint = ["python", "python3", "python"]
        .iter()
        .map(which::which)
        .find_map(|x| x.ok()) 
        .ok_or_else(|| static_err("cannot find a python intepreter!"))?;

    let mut cmd = Command::new(pyint);
    cmd.current_dir(current_dir()?);
    cmd.arg(path.as_ref().as_os_str());
    Ok(cmd)
}



pub type Success<'a> = (&'a Path, String);
pub type Failure<'a> = (&'a Path, ExitStatus, String);
pub type Execution<'a> = Result<Success<'a>, Failure<'a>>;

pub fn generate_input(args: &Cli) -> Execution {
    execute_prog(args.generator.as_path())
}

pub fn execute_prog(path: & Path) -> Execution
{
    let gen = get_command(path)
        .expect("cannot open program")
        .output()
        .expect("cannot start program");

    let gen_errors = String::from_utf8(gen.stderr).expect("error parsing string");
    if !gen_errors.is_empty() || !gen.status.success()  {
        Err((path, gen.status, gen_errors))
    } else {
        Ok((path, String::from_utf8(gen.stdout).expect("cannot parse string")))
    }
}

pub fn start_prog_input<P>(path: P, input: &str) -> Child
where P: AsRef<Path>
{
    let mut gen = get_command(path)
        .expect("cannot open generator")
        .stdin(Stdio::piped())
        .stderr(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("cannot start generator program");

    let mut stdin = gen.stdin.take().expect("failed to open stdin");
    stdin.write_all(input.as_bytes()).expect("failed to write input!");
    gen
}

pub fn end_child_output(ch: Child, path: & Path) -> Execution
{
    let out = ch.wait_with_output().expect("failed to read stdout and stderr");
    let gen_errors = String::from_utf8(out.stderr).expect("error parsing string");
    if !gen_errors.is_empty() || !out.status.success() {
        Err((path, out.status, gen_errors))
    } else {
        Ok((path, String::from_utf8(out.stdout).expect("cannot parse string")))
    }
}

pub fn execute_prog_input<'a>(path: &'a Path, input: &str) -> Execution<'a>
{
    let gen = start_prog_input(path, input);
    end_child_output(gen, path)
}

pub fn execute_progs_input<'a, I>(paths: I, input: &str) -> Vec<Execution<'a>>
where I: Iterator<Item = &'a Path>, 
{
    paths
        .map(|path| (path, start_prog_input(path, input)))
        .map(|(path, child)| end_child_output(child, path))
        .collect()
}

pub enum Round<'a>{
    GeneratorFail(Failure<'a>),
    ReferenceFails(String, Vec<Failure<'a>>),
    ProgramFail(String, Failure<'a>),
    Success(String, Success<'a>, Vec<Success<'a>>),
}

pub fn run_round(args: &Cli) -> Round {
    let inp = generate_input(args);
    if let Err(x) = inp { return Round::GeneratorFail(x); }
    let inp = unsafe{ inp.unwrap_unchecked() };

    let prg = execute_prog_input(args.program.as_path(), inp.1.as_str());
    if let Err(x) = prg { return Round::ProgramFail(inp.1, x); }
    let prq = unsafe{ prg.unwrap_unchecked() };

    let refs = args.reference.iter()
        .map(PathBuf::as_path);
    let refs = execute_progs_input(refs, inp.1.as_str());
    if refs.iter().any(|x| x.is_err()) { 
        let r = refs.into_iter().filter_map(|x| x.err()).collect();
        Round::ReferenceFails(inp.1, r)
    } else { 
        let r = refs.into_iter().map(|x| unsafe{ x.unwrap_unchecked() }).collect();
        Round::Success(inp.1, prq, r)
    }
}

pub enum Mismatch<'a>{
    AllMatch,
    RefMismatch(Vec<Success<'a>>),
    ProgMismatch(Success<'a>, Vec<Success<'a>>),
}

pub fn test_mismatch<'a>(prog: Success<'a>, refs: Vec<Success<'a>>) -> Mismatch<'a> {
    if refs.iter().all(|x| x.1 == prog.1) { return Mismatch::AllMatch; }

    // just a random string which will should never be the output
    // indeed it is impossible that the \0 is at the beginning
    let bad = String::from("\0\n\t@"); 
    let bad = refs
        .iter()
        .map(|x| &x.1)
        .filter(|x| x != &&prog.1)
        .reduce(|a, i| if a == i { a } else { &bad } )
        .expect("this iterator should not be empty!")
        == &bad;
        
    if bad { Mismatch::RefMismatch(refs) }
    else { 
        let refs = refs.into_iter().filter(|x| x.1 != prog.1).collect();
        Mismatch::ProgMismatch(prog, refs) 
    }
}

