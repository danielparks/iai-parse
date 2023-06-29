//! iai-parse executable.

// Most lint configuration is in lints.toml, but it doesn’t support forbid.
#![forbid(unsafe_code)]

use anyhow::Context;
use clap::Parser;
use git2::{ObjectType, Repository};
use indexmap::{IndexMap, IndexSet};
use std::collections::HashSet;
use std::convert::From;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::exit;
use std::result::Result;

/// Parameters to configure executable.
#[derive(Debug, clap::Parser)]
#[clap(version, about)]
struct Params {
    /// File(s) to parse.
    input: Vec<PathBuf>,

    /// Git revisions to check, e.g. main..HEAD.
    #[clap(long, short('r'), value_name = "REVSPEC")]
    git_revs: Vec<String>,

    /// Path to git repo (defaults to consulting $GIT_DIR then searching the
    /// working directory and its parents)
    #[clap(long, value_name = "PATH")]
    git_repo: Option<PathBuf>,
}

/// A table of [`Column`]s that can be written to CSV.
///
/// Each column represent a benchmark run.
#[derive(Clone, Debug, Default)]
struct Table {
    /// An ordered map of column names to [`Column`]s.
    columns: IndexMap<Vec<u8>, Column>,
}

/// A column of values from a benchmark run.
#[derive(Clone, Debug, Default)]
struct Column {
    /// Ordered map of individual benchmark names (e.g. a function name) to
    /// ordered map of benchmark parameter names to values, e.g.
    /// `b"Instructions" : b"451"`.
    benchmarks: IndexMap<Vec<u8>, IndexMap<Vec<u8>, Vec<u8>>>,
}

impl Table {
    /// Get a [`Column`] by `name`.
    ///
    /// This will create a column if it doesn’t already exist.
    pub fn column(&mut self, name: &[u8]) -> &mut Column {
        self.columns
            .entry(name.to_vec())
            .or_insert_with(Column::default)
    }

    /// Get a vector of byte strings representing headers in the table.
    ///
    /// The first two will always be `b"benchmark"` and `b"parameter"`; the
    /// following entries will the names of the columns.
    pub fn headers(&self) -> Vec<Vec<u8>> {
        let mut headers = Vec::with_capacity(
            // Very unlikely to fail.
            self.columns.len().checked_add(2).unwrap(),
        );
        headers.push(b"benchmark".to_vec());
        headers.push(b"parameter".to_vec());
        headers.extend(self.columns.keys().cloned());
        headers
    }

    /// Return all unique pairs of individual benchmark names (e.g. function
    /// names) and parameter names (e.g. “Instructions”).
    pub fn benchmarks_and_parameters(&self) -> IndexSet<(Vec<u8>, Vec<u8>)> {
        self.columns
            .values()
            .flat_map(|column| {
                column.benchmarks().iter().flat_map(
                    |(benchmark, parameter_map)| {
                        parameter_map.keys().map(|parameter| {
                            (benchmark.clone(), parameter.clone())
                        })
                    },
                )
            })
            .collect()
    }

    /// Write table contents as CSV.
    ///
    /// # Errors
    ///
    /// Returns IO errors from trying to write data.
    pub fn write_csv<W: io::Write>(&self, writer: W) -> anyhow::Result<()> {
        let mut csv_writer = csv::Writer::from_writer(writer);
        csv_writer.write_record(self.headers())?;

        let empty = b"".to_vec();
        for (benchmark, parameter) in self.benchmarks_and_parameters().iter() {
            let start = [benchmark.clone(), parameter.clone()];
            csv_writer.write_record(start.into_iter().chain(
                self.columns.values().map(|column| {
                    column.get(benchmark, parameter).unwrap_or(&empty).clone()
                }),
            ))?;
        }
        Ok(())
    }
}

impl Column {
    /// Get a parameter value from an individual benchmark within this run.
    pub fn get(
        &self,
        benchmark: &Vec<u8>,
        parameter: &Vec<u8>,
    ) -> Option<&Vec<u8>> {
        self.benchmarks
            .get(benchmark)
            .and_then(|parameter_map| parameter_map.get(parameter))
    }

    /// Set a parameter value for an individual benchmark within this run.
    pub fn set(&mut self, benchmark: &[u8], parameter: &[u8], value: &[u8]) {
        let parameter_map = self
            .benchmarks
            .entry(benchmark.to_vec())
            .or_insert_with(IndexMap::new);
        // FIXME: check if a value already exists?
        parameter_map.insert(parameter.to_vec(), value.to_vec());
    }

    /// Get the ordered map of benchmark names to parameter maps.
    pub const fn benchmarks(
        &self,
    ) -> &IndexMap<Vec<u8>, IndexMap<Vec<u8>, Vec<u8>>> {
        &self.benchmarks
    }
}

fn main() {
    if let Err(error) = cli(Params::parse()) {
        eprintln!("Error: {error:#}");
        exit(1);
    }
}

/// Do real work and return errors so that they can be reported nicely.
///
/// # Errors
///
/// This returns various errors that should be reported to the user.
fn cli(params: Params) -> anyhow::Result<()> {
    if params.git_revs.is_empty() {
        return parse_in_working_tree(params);
    }

    let repo = if let Some(repo_path) = params.git_repo {
        Repository::open(repo_path)?
    } else {
        Repository::open_from_env()?
    };

    let mut table = Table::default();
    for revspec_str in params.git_revs {
        for commit in revspec_parse(&repo, &revspec_str)? {
            let commit = commit?;
            let column = table.column(
                format!(
                    "{} {}",
                    abbrev(commit.id()),
                    commit.summary().unwrap_or("")
                )
                .as_bytes(),
            );
            let tree = commit.tree()?;
            for path in &params.input {
                let entry = match tree.get_path(path) {
                    Ok(entry) => entry,
                    Err(error) => {
                        if error.code() == git2::ErrorCode::NotFound {
                            eprintln!(
                                "{:?} not found in {}",
                                path.display(),
                                commit.id()
                            );
                            continue;
                        }
                        return Err(error.into());
                    }
                };

                let object = entry.to_object(&repo)?;
                match object.kind() {
                    Some(ObjectType::Blob) => {
                        parse(object.peel_to_blob()?.content(), column);
                    }
                    Some(ObjectType::Tree) => eprintln!(
                        "{:?} is directory in {}",
                        path.display(),
                        commit.id()
                    ),
                    Some(kind) => eprintln!(
                        "{:?} is {kind} in {}",
                        path.display(),
                        commit.id()
                    ),
                    None => eprintln!(
                        "{:?} is unknown in {}",
                        path.display(),
                        commit.id()
                    ),
                }
            }
        }
    }

    table.write_csv(io::stdout())?;

    Ok(())
}

/// Parse paths from the filesystem (as opposed to from git history).
fn parse_in_working_tree(params: Params) -> anyhow::Result<()> {
    let mut table = Table::default();
    {
        let column = table.column(b"value");
        for path in params.input {
            parse(read(path)?, column);
        }
    }

    table.write_csv(io::stdout())?;

    Ok(())
}

/// Abbreviate a git hash to 7 characters.
// FIXME: should we be checking that the abbreviations are unique?
fn abbrev(oid: git2::Oid) -> String {
    let mut hash = oid.to_string();
    hash.truncate(7);
    hash
}

/// Generate an iterator of commits from a git repo and revspec.
///
/// The iterator will be in order from oldest to newest.
///
/// # Errors
///
/// Returns an error if revspec couldn’t be parsed, or the commits couldn’t be
/// found in the repository.
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
            // Range of revisions. This works like `git log` it loads the
            // history of both `from` and `to` and removes everything in the
            // `from` list from the `to` list.
            let mut from_walker = repo.revwalk()?;
            from_walker.push(from.id())?;
            let from_oids: HashSet<git2::Oid> =
                from_walker.filter_map(Result::ok).collect();

            let mut to_walker = repo.revwalk()?;
            to_walker.set_sorting(git2::Sort::REVERSE)?;
            to_walker.push(to.id())?;

            Ok(Box::new(
                to_walker
                    .filter(move |oid_result| {
                        oid_result
                            .as_ref()
                            .map(|oid| !from_oids.contains(oid))
                            .unwrap_or(true)
                    })
                    // Load commit objects
                    .map(|oid_result| {
                        oid_result.and_then(|oid| repo.find_commit(oid))
                    }),
            ))
        }
    }
}

/// Read the passed `path` to a byte string.
///
/// # Errors
///
/// Returns an error if the file couldn’t be read. The error message will start
/// with “Failed to read path/to/file”.
fn read<P: AsRef<Path>>(path: P) -> anyhow::Result<Vec<u8>> {
    let path = path.as_ref();
    fs::read(path).with_context(|| format!("Failed to read {}", path.display()))
}

/// Parse a byte string and write it to the passed [`Column`].
fn parse<B>(input: B, column: &mut Column)
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
}

/// Find the subslice with leading spaces removed.
fn trim_leading_spaces(input: &[u8]) -> &[u8] {
    if let Some(start) = input.iter().position(|&c| c != b' ') {
        &input[start..]
    } else {
        input
    }
}

/// Find the first sequence of non-space characters as a subslice.
fn parse_parameter_value(input: &[u8]) -> &[u8] {
    let mut iter = input.iter();
    let start = iter
        .position(|&c| c != b' ')
        .expect("parameter value empty");
    if let Some(end) = iter.position(|&c| c == b' ') {
        // start + end must be a valid index within input.
        #[allow(clippy::arithmetic_side_effects)]
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
            let column = table.column(b"value");
            let input = read("tests/corpus/iai-output-short.txt")?;
            parse(input, column);
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
