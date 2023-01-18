use anyhow::Context;
use clap::Parser;
use git2::Repository;
use std::convert::From;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::exit;
use std::result::Result;

#[derive(Debug, clap::Parser)]
#[clap(version, about)]
struct Params {
    /// File(s) to parse.
    input: Vec<PathBuf>,

    /// Git revisions to check, e.g. main..HEAD.
    #[clap(long, short('r'), value_name = "REVSPEC")]
    git_revs: Vec<String>,
}

struct Receiver<W: io::Write> {
    writer: csv::Writer<W>,
}

impl<W: io::Write> Receiver<W> {
    pub fn set(
        &mut self,
        benchmark: &[u8],
        parameter: &[u8],
        value: &[u8],
    ) -> anyhow::Result<()> {
        self.writer.write_record([benchmark, parameter, value])?;
        Ok(())
    }
}

impl<W: io::Write> From<csv::Writer<W>> for Receiver<W> {
    fn from(csv_writer: csv::Writer<W>) -> Self {
        Receiver { writer: csv_writer }
    }
}

impl<W: io::Write> From<W> for Receiver<W> {
    fn from(writer: W) -> Self {
        Receiver {
            writer: csv::Writer::from_writer(writer),
        }
    }
}

fn main() {
    if let Err(error) = cli(Params::parse()) {
        eprintln!("Error: {:#}", error);
        exit(1);
    }
}

fn cli(params: Params) -> anyhow::Result<()> {
    if !params.git_revs.is_empty() {
        let repo = Repository::open_from_env()?;
        for revspec_str in params.git_revs {
            for commit in revspec_parse(&repo, &revspec_str)? {
                let commit = commit?;
                println!(
                    "{} {}",
                    abbrev(commit.id()),
                    commit.summary().unwrap_or("")
                );
            }
        }
        return Ok(());
    }

    let mut writer = csv::Writer::from_writer(io::stdout());
    writer.write_record([
        &b"benchmark"[..],
        &b"parameter"[..],
        &b"value"[..],
    ])?;

    let mut receiver = Receiver::from(writer);

    for path in params.input {
        parse(read(path)?, &mut receiver)?;
    }

    Ok(())
}

// FIXME: should we be checking that the abbreviations are unique?
fn abbrev(oid: git2::Oid) -> String {
    let mut hash = oid.to_string();
    hash.truncate(7);
    hash
}

fn revspec_parse<'r>(
    repo: &'r Repository,
    revspec_str: &str,
) -> anyhow::Result<
    Box<dyn Iterator<Item = Result<git2::Commit<'r>, git2::Error>> + 'r>,
> {
    let revspec = repo.revparse(revspec_str)?;
    match (revspec.from(), revspec.to()) {
        (None, None) => {
            anyhow::bail!("Got no revisions from revspec {revspec_str:?}");
        }
        (None, Some(_to)) => {
            anyhow::bail!(
                "Unsure how to handle revspec with only .to(): {revspec_str:?}"
            );
        }
        (Some(from), None) => {
            // Single revision.
            let mut walker = repo.revwalk()?;
            walker.push(from.id())?;
            Ok(Box::new(walker.take(1).map(|oid_result| {
                oid_result.and_then(|oid| repo.find_commit(oid))
            })))
        }
        (Some(from), Some(to)) => {
            // Range of revisions.
            let mut walker = repo.revwalk()?;
            walker.push(to.id())?;
            let from_oid = from.id();
            Ok(Box::new(walker.map_while(move |oid_result| {
                match oid_result {
                    Ok(oid) => {
                        if oid == from_oid {
                            None // Stop iterating
                        } else {
                            Some(repo.find_commit(oid))
                        }
                    }
                    Err(error) => Some(Err(error)),
                }
            })))
        }
    }
}

fn read<P: AsRef<Path>>(path: P) -> anyhow::Result<Vec<u8>> {
    let path = path.as_ref();
    fs::read(path).with_context(|| format!("Failed to read {}", path.display()))
}

fn parse<B, W>(input: B, receiver: &mut Receiver<W>) -> anyhow::Result<()>
where
    B: AsRef<[u8]>,
    W: io::Write,
{
    let mut benchmark = Vec::<u8>::new();

    for line in input.as_ref().split(|&c| c == b'\n' || c == b'\r') {
        match line {
            [] => {} // Empty line; skip.
            [b' ', ..] => {
                // Parameter line ("  parameter:  value (change)").
                let line = trim_leading_spaces(line);
                let mut iter = line.splitn(2, |&c| c == b':');
                let parameter = iter.next().expect("parameter name missing");
                let value = parse_parameter_value(
                    iter.next().expect("parameter value missing"),
                );

                receiver.set(&benchmark, parameter, value)?;
            }
            [..] => {
                // A line not starting with a space.
                benchmark = line.to_vec();
            }
        }
    }

    Ok(())
}

fn trim_leading_spaces(input: &[u8]) -> &[u8] {
    if let Some(start) = input.iter().position(|&c| c != b' ') {
        &input[start..]
    } else {
        input
    }
}

fn parse_parameter_value(input: &[u8]) -> &[u8] {
    let mut iter = input.iter();
    let start = iter
        .position(|&c| c != b' ')
        .expect("parameter value empty");
    if let Some(end) = iter.position(|&c| c == b' ') {
        &input[start..=start + end]
    } else {
        &input[start..]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use assert2::check;

    #[test]
    fn simple() {
        let mut output: Vec<u8> = Vec::new();
        let mut receiver = Receiver::from(&mut output);
        let input = read("tests/corpus/iai-output-short.txt").unwrap();
        parse(input, &mut receiver).unwrap();
        drop(receiver);
        check!(output == read("tests/corpus/iai-output-short.csv").unwrap());
    }
}
