use anyhow::Context;
use clap::Parser;
use git2::{ObjectType, Repository};
use indexmap::{IndexMap, IndexSet};
use std::convert::From;
use std::fs;
use std::io::{self, Write};
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

#[derive(Clone, Debug, Default)]
struct Table {
    columns: IndexMap<Vec<u8>, Column>,
}

#[derive(Clone, Debug, Default)]
struct Column {
    benchmarks: IndexMap<Vec<u8>, IndexMap<Vec<u8>, Vec<u8>>>,
}

impl Table {
    pub fn column(&mut self, name: &[u8]) -> &mut Column {
        self.columns
            .entry(name.to_vec())
            .or_insert_with(Column::default)
    }

    pub fn headers(&self) -> Vec<Vec<u8>> {
        let mut headers = Vec::with_capacity(2 + self.columns.len());
        headers.push(b"benchmark".to_vec());
        headers.push(b"parameter".to_vec());
        headers.extend(self.columns.keys().cloned());
        headers
    }

    pub fn benchmarks_and_parameters(
        &self,
    ) -> IndexMap<Vec<u8>, IndexSet<Vec<u8>>> {
        let mut benchmarks: IndexMap<Vec<u8>, IndexSet<Vec<u8>>> =
            IndexMap::new();
        self.columns.values().for_each(|column| {
            column
                .benchmarks()
                .iter()
                .for_each(|(name, parameter_map)| {
                    let parameters = benchmarks
                        .entry(name.clone())
                        .or_insert_with(IndexSet::new);
                    parameter_map.keys().for_each(|parameter| {
                        parameters.insert(parameter.clone());
                    });
                });
        });

        benchmarks
    }

    pub fn write_csv<W: io::Write>(&self, writer: W) -> anyhow::Result<()> {
        let mut csv_writer = csv::Writer::from_writer(writer);
        csv_writer.write_record(self.headers())?;
        for (benchmark, parameters) in self.benchmarks_and_parameters().iter() {
            for parameter in parameters {
                csv_writer.write_record([
                    &benchmark,
                    &parameter,
                    &b"".to_vec(),
                ])?;
            }
        }
        Ok(())
    }
}

impl Column {
    pub fn set(&mut self, benchmark: &[u8], parameter: &[u8], value: &[u8]) {
        let parameter_map = self
            .benchmarks
            .entry(benchmark.to_vec())
            .or_insert_with(IndexMap::new);
        // FIXME: check if a value already exists?
        parameter_map.insert(parameter.to_vec(), value.to_vec());
    }

    pub fn benchmarks(&self) -> &IndexMap<Vec<u8>, IndexMap<Vec<u8>, Vec<u8>>> {
        &self.benchmarks
    }
}

fn main() {
    if let Err(error) = cli(Params::parse()) {
        eprintln!("Error: {:#}", error);
        exit(1);
    }
}

fn cli(params: Params) -> anyhow::Result<()> {
    if params.git_revs.is_empty() {
        return parse_in_working_tree(params);
    }

    let repo = Repository::open_from_env()?;
    for revspec_str in params.git_revs {
        for commit in revspec_parse(&repo, &revspec_str)? {
            let commit = commit?;
            println!(
                "{} {}",
                abbrev(commit.id()),
                commit.summary().unwrap_or("")
            );
            let tree = commit.tree()?;
            for path in &params.input {
                let entry = match tree.get_path(path) {
                    Ok(entry) => entry,
                    Err(error) => {
                        if error.code() == git2::ErrorCode::NotFound {
                            println!("  {} not found", path.display());
                            continue;
                        }
                        return Err(error.into());
                    }
                };
                println!("  {:?}", entry.name());
                let object = entry.to_object(&repo)?;
                match object.kind() {
                    None => println!("  {} is unknown", path.display()),
                    Some(ObjectType::Blob) => {
                        let blob = object.peel_to_blob()?;
                        io::stdout().write_all(blob.content())?;
                    }
                    Some(ObjectType::Tree) => {
                        println!("  {} is directory", path.display());
                    }
                    Some(kind) => {
                        println!("  {} is {kind}", path.display());
                    }
                }
            }
        }
    }
    Ok(())
}

fn parse_in_working_tree(params: Params) -> anyhow::Result<()> {
    let mut table = Table::default();
    {
        let column = table.column(b"value");
        for path in params.input {
            parse(read(path)?, column)?;
        }
    }

    table.write_csv(io::stdout())?;

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

fn parse<B>(input: B, column: &mut Column) -> anyhow::Result<()>
where
    B: AsRef<[u8]>,
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

                column.set(&benchmark, parameter, value);
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
    use pretty_assertions::assert_str_eq;

    #[test]
    fn simple() -> anyhow::Result<()> {
        let mut table = Table::default();
        {
            let mut column = table.column(b"value");
            let input = read("tests/corpus/iai-output-short.txt")?;
            parse(input, &mut column)?;
        }

        let mut output: Vec<u8> = Vec::new();
        table.write_csv(&mut output)?;

        assert_str_eq!(
            String::from_utf8(output)?,
            String::from_utf8(read("tests/corpus/iai-output-short.csv")?)?,
        );
        Ok(())
    }
}
