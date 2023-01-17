use anyhow::Context;
use clap::Parser;
use std::convert::From;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::exit;

#[derive(Debug, clap::Parser)]
#[clap(version, about)]
struct Params {
    /// File(s) to parse.
    input: Vec<PathBuf>,
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
