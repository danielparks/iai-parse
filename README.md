# Convert iai benchmark output to CSV

Simple tool to read [iai] output and produce CSV. It can read multiple versions
of a file from Git history and produce a CSV file that summarizes the changes.

## Installation and usage

```sh
cargo binstall iai-parse || cargo install iai-parse
```

For every commit you want to benchmark, run your iai benchmarks, save the output
to a file, and commit it. For example:

```
❯ cargo bench --quiet iai | tee iai-output.txt
iai_bench1
  Instructions:                 358 (No change)
  L1 Accesses:                  402 (No change)
  L2 Accesses:                    4 (No change)
  RAM Accesses:                  28 (No change)
  Estimated Cycles:            1402 (No change)

❯ git commit --amend --no-edit iai-output.txt
```

(I put “iai” in the name of all my iai benchmarks so I can easily limit bench
runs to just them.)

Run `iai-parse` on the revisions you care about to get a CSV file with a summary
of the changes:

```
❯ iai-parse -r main..my_branch iai-output.txt
benchmark,parameter,4a1953a First change,4cbe905 Second change
iai_escape_text_clean_small,Instructions,401,358
iai_escape_text_clean_small,L1 Accesses,404,402
iai_escape_text_clean_small,L2 Accesses,4,3
iai_escape_text_clean_small,RAM Accesses,30,29
iai_escape_text_clean_small,Estimated Cycles,1498,1432
```

## Rust Crate

[![Crates.io](https://img.shields.io/crates/v/iai-parse)][crates.io]

## License

This project dual-licensed under the Apache 2 and MIT licenses. You may choose
to use either.

  * [Apache License, Version 2.0](LICENSE-APACHE)
  * [MIT license](LICENSE-MIT)

### Contributions

Unless you explicitly state otherwise, any contribution you submit as defined
in the Apache 2.0 license shall be dual licensed as above, without any
additional terms or conditions.

[docs.rs]: https://docs.rs/iai-parse/latest/iai_parse/
[crates.io]: https://crates.io/crates/iai-parse
[iai]: https://crates.io/crates/iai
