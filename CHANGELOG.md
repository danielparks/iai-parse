# Change log

All notable changes to this project will be documented in this file.

## Release 0.1.2 (2023-06-21)

### Security fixes

* Upgrade [git2] dependency to 0.17.2 to fix a [security vulnerability in its
  handling of SSH keys][GHSA-m4ch-rfv5-x5g3]. This was unlikely to affect
  iai-parse since it doesn’t fetch data from, or otherwise interact with, remote
  repositories.

[git2]: https://crates.io/crates/git2
[GHSA-m4ch-rfv5-x5g3]: https://github.com/rust-lang/git2-rs/security/advisories/GHSA-m4ch-rfv5-x5g3

## Release 0.1.1 (2023-01-19)

### Bug fixes

* Release process: fix broken binary upload.

## Release 0.1.0 (2023-01-19)

### Features

* Initial release.
