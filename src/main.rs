use clap::Parser;
use csv::Writer;
use std::fs;
use std::io;
use std::path::PathBuf;
use std::process::exit;

#[derive(Debug, clap::Parser)]
#[clap(version, about)]
struct Params {
    /// File(s) to parse.
    input: Vec<PathBuf>,
}

struct Receiver<W: io::Write> {
    writer: Writer<W>,
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

fn main() {
    if let Err(error) = cli(Params::parse()) {
        eprintln!("Error: {:#}", error);
        exit(1);
    }
}

fn cli(params: Params) -> anyhow::Result<()> {
    let mut writer = Writer::from_writer(io::stdout());
    writer.write_record([
        &b"benchmark"[..],
        &b"parameter"[..],
        &b"value"[..],
    ])?;

    let mut receiver = Receiver { writer };

    for path in params.input {
        parse(fs::read(path)?, &mut receiver)?;
    }

    Ok(())
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
