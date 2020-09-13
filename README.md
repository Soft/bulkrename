# bulkrename

[![Build Status](https://travis-ci.org/Soft/bulkrename.svg?branch=master)](https://travis-ci.org/Soft/bulkrename)
[![GitHub release](https://img.shields.io/github/release/Soft/bulkrename.svg)](https://github.com/Soft/bulkrename/releases)
[![Latest Version](https://img.shields.io/crates/v/bulkrename.svg)](https://crates.io/crates/bulkrename)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

`bulkrename` is a tool for renaming large numbers of files.

`bulkrename` accepts file paths either via command line arguments or via
standard input. When invoked, `bulkrename` opens a file in `EDITOR` with the
input paths. After user exits `EDITOR`, `bulkrename` will rename all the input
files using the file names from the file as their new names.

If you are familiar with the `bulkrename` command of the
[Ranger](https://github.com/ranger/ranger) file manager, this program works
basically the same way.

## Installation

Pre-built binaries can be downloaded from [GitHub
Releases](https://github.com/Soft/bulkrename/releases). These should work on any
64-bit Linux system.

Alternatively, `bulkrename` can be installed using
[Cargo](https://doc.rust-lang.org/cargo/):

``` shell
cargo install bulkrename
```

## Usage

```
usage: bulkrename [-h|--help] [FILE]...
bulkrename is a tool for renaming large numbers of files.

options:
  -h, --help:        display this help
  -r, --replace:     allow replacing existing files
  -q, --quiet:       do not display information about operations being performed
```

## Examples

``` shell
bulkrename file-1.txt file-2.txt file-3.txt
find examples | bulkrename
```

