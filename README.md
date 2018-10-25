dotlock
=======

[![Crate](https://img.shields.io/crates/v/dotlock.svg)](https://crates.io/crates/dotlock)
[![Build Status](https://travis-ci.org/bruceg/dotlock.svg?branch=master)](https://travis-ci.org/bruceg/dotlock)

This crate contains support for creating lock files as are used on
various UNIX type systems. This is similar to the `lockfile` program
from [procmail](http://www.procmail.org) or the `dotlockfile` program
from [liblockfile](https://github.com/miquels/liblockfile).

[Documentation](https://docs.rs/dotlock/)

Usage
-----

Add this to your `Cargo.toml`:
```toml
[dependencies]
dotlock = "0"
```

...and this to your crate root:
```rust
extern crate dotlock;
```

Example
-------

```rust
extern crate dotlock;
use std::fs::File;
use std::io::{Write, Read, Seek, SeekFrom};

fn main() {
    let mut lock = dotlock::Dotlock::create("some.file.lock").unwrap();
    writeln!(lock, "Do not touch this file!").unwrap();
}
```
