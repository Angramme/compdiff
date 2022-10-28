pub mod cli;

use std::{fmt, process::{Command, Stdio, ExitStatus}, io::{BufWriter, Write}, path::{Path, PathBuf}, iter, env::current_dir};
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

#[derive(Debug, Clone)]
pub enum RunError{
    StdErrs(String, Vec<(PathBuf, ExitStatus, String)>),
}

impl Error for RunError {}
impl fmt::Display for RunError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::StdErrs(_, xs) => for (p, s, e) in xs {
                write!(f, "program {} failed with status {} and the error: {}", p.display(), s, e)?;
            }
        };
        Ok(())
    }
}

pub fn run_round(args: &Cli) -> Result<(String, Vec<(&Path, String)>), RunError>  {
    let gen = get_command(args.generator.as_path())
        .expect("cannot open generator")
        .output()
        .expect("cannot start generator program");

    let mut progs: Vec<_> = 
        iter::once(args.program.as_path())
        .chain(args.reference
            .iter()
            .map(PathBuf::as_path))
        .map(get_command)
        .map(|x| x
            .expect("cannot open program")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("cannot start the program")
        )
        .collect();

    let gen_errors = String::from_utf8(gen.stderr).expect("error parsing string");
    if !gen_errors.is_empty() {
        return Err(RunError::StdErrs(String::new(), vec![(
            args.program.clone(), 
            gen.status, 
            gen_errors
        )]));
    }
    
    let input = String::from_utf8(gen.stdout).expect("error parsing string");
    {
        let mut writers: Vec<_> = progs
            .iter_mut()
            .map(|x| x.stdin.as_mut().expect("cannot access stdin!"))
            .map(BufWriter::new)
            .collect();
    
        for line in input.lines() {
            let line = String::from(line) + "\n";
            for writer in &mut writers{
                writer.write_all(line.as_bytes()).expect("error writing to the program!");
            }
        }
    }

    let outps: Vec<_> = progs
        .into_iter()
        .map(|x| x
            .wait_with_output()
            .expect("program cannot terminate"))
        .collect();
    
    let stderrs: Vec<_> = outps
        .iter()
        .zip(iter::once(args.program.clone())
            .chain(args.reference.iter().cloned()))
        .filter(|(x, _)| !x.stderr.is_empty())
        .map(|(x, p)| (p, x.status, String::from_utf8(x.stderr.clone()).expect("cannot parse string.")))
        .collect();

    if !stderrs.is_empty() {
        return Err(RunError::StdErrs(input, stderrs));
    }

    Ok((input, 
        iter::once(&args.program)
        .chain(args.reference.iter())
        .map(PathBuf::as_path)
        .zip(outps
            .into_iter()
            .map(|o| String::from_utf8(o.stdout).expect("cannot parse string!"))
        ).collect()))
}

