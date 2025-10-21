# Change log

All notable changes to this project will be documented in this file.

## main branch

## Release 0.1.6 (2025-10-21)

* Improvements to release process; no functional changes.

## Release 0.1.5 (2025-02-12)

* Bump version to fix release workflow.

## Release 0.1.4 (2025-02-12)

### Security fixes

* Upgrade indirect dependency [idna] to fix a [security
  vulnerability][RUSTSEC-2024-0421] in domain handling. This does not
  appear to affect iai-parse.

[idna]: https://crates.io/crates/idna
[RUSTSEC-2024-0421]: https://rustsec.org/advisories/RUSTSEC-2024-0421

## Release 0.1.3 (2024-02-12)

### Security fixes

* Upgrade [git2] dependency to 0.18.2 to fix [security vulnerabilities in
  libgit2][GHSA-22q8-ghmq-63vf], including in revision parsing. These do not
  appear to affect iai-parse.
* Upgrade indirect dependency [rustix] to fix a [security
  vulnerability][GHSA-c827-hfw6-qwvm] in directory iterators. This does not
  appear to affect iai-parse.

[git2]: https://crates.io/crates/git2
[GHSA-22q8-ghmq-63vf]: https://github.com/advisories/GHSA-22q8-ghmq-63vf
[rustix]: https://crates.io/crates/rustix
[GHSA-c827-hfw6-qwvm]: https://github.com/advisories/GHSA-c827-hfw6-qwvm

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
