pub mod cli;

use std::{process::{Command, Stdio, Child, Output}, io::Write, path::{Path, PathBuf}, env::current_dir, time::Duration};
use std::ffi::OsStr;
use std::error::Error;
use cli::Cli;
use process_control::ChildExt;
use process_control::Control;
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



// pub type Failure<'a> = (&'a Path, ExitStatus, String);
pub enum Failure<'a> {
    Prog(&'a Path, String, String),
    TimeLimit(&'a Path),
}
pub type Success<'a> = (&'a Path, String);
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
        Err(Failure::Prog(path, gen.status.to_string(), gen_errors))
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
        .expect("cannot start program");

    let mut stdin = gen.stdin.take().expect("failed to open stdin");
    stdin.write_all(input.as_bytes()).expect("failed to write input!");
    gen
}

pub fn start_prog_input_limits<P>(path: P, input: &str, tlimit: Option<Duration>, mlimit: Option<usize>) -> Option<process_control::Output>
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
    
    let mut gen = gen
        .controlled_with_output();

    if let Some(t) = tlimit {
        gen = gen.time_limit(t);
    }
    if let Some(m) = mlimit {
        gen = gen.memory_limit(m);
    }
        
    gen
        .terminate_for_timeout()
        .wait()
        .expect("couldn't wait for the programme!")
}

pub fn output_to_execution(out: Output, path: & Path) -> Execution
{
    let gen_errors = String::from_utf8(out.stderr).expect("error parsing string");
    if !gen_errors.is_empty() || !out.status.success() {
        Err(Failure::Prog(path, out.status.to_string(), gen_errors))
    } else {
        Ok((path, String::from_utf8(out.stdout).expect("cannot parse string")))
    }
}

pub fn execute_prog_input_limits<'a>(path: &'a Path, input: &str, tlimit: Option<Duration>, mlimit: Option<usize>) -> Execution<'a>
{
    let out = start_prog_input_limits(path, input, tlimit, mlimit);    
    match out {
        None => Err(Failure::TimeLimit(path)),
        Some(out) => {
            let gen_errors = String::from_utf8(out.stderr).expect("error parsing string");
            if !gen_errors.is_empty() || !out.status.success() {
                Err(Failure::Prog(path, out.status.to_string(), gen_errors))
            } else {
                Ok((path, String::from_utf8(out.stdout).expect("cannot parse string")))
            }
        }
    }
}

pub fn execute_prog_input<'a>(path: &'a Path, input: &str) -> Execution<'a>
{
    let gen = start_prog_input(path, input);
    let out = gen.wait_with_output().expect("failed to read stdout and stderr");
    output_to_execution(out, path)
}

pub fn execute_progs_input<'a, I>(paths: I, input: &str) -> Vec<Execution<'a>>
where I: Iterator<Item = &'a Path>, 
{
    paths
        .map(|path| (path, start_prog_input(path, input)))
        .map(|(path, child)| (path, child.wait_with_output().expect("failed to read stdout and stderr")))
        .map(|(path, child)| output_to_execution(child, path))
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

    let prg = if args.time_limit.is_none() { 
        execute_prog_input(args.program.as_path(), inp.1.as_str())
    } else {
        let tl = args.time_limit.map(Duration::from_secs_f64);
        let mm = args.memory_limit.map(|x| x*1000); // convert from kilobytes to bytes
        execute_prog_input_limits(args.program.as_path(), inp.1.as_str(), tl, mm)
    };
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
    let bad = String::from("\0\0\0\t@"); 
    let bad = refs
        .iter()
        .map(|x| &x.1)
        .filter(|x| x != &&prog.1)
        .reduce(|a, i| if a == i { a } else { &bad } )
        .unwrap()
        == &bad;
        
    if bad { Mismatch::RefMismatch(refs) }
    else { 
        let refs = refs.into_iter().filter(|x| x.1 != prog.1).collect();
        Mismatch::ProgMismatch(prog, refs) 
    }
}

