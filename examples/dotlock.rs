extern crate dotlock;
#[macro_use]
extern crate structopt;

use dotlock::*;
use std::ffi::OsString;
use std::io::Write;
use std::path::PathBuf;
use std::process::{exit, Command};
use std::time::Duration;
use structopt::StructOpt;

#[derive(Debug, StructOpt)]
#[structopt(name = "dotlock", about = "Utility to create .lock files")]
struct Opt {
    /// Set retry delay
    #[structopt(short = "d", long = "delay", default_value = "5 seconds")]
    pause: f64,
    /// Set number of retries
    #[structopt(short = "n", long = "tries", default_value = "10")]
    tries: usize,
    /// Lock file
    #[structopt(parse(from_os_str))]
    lockfile: PathBuf,
    /// Program to run
    #[structopt(parse(from_os_str))]
    command: OsString,
    /// Command arguments
    #[structopt(parse(from_os_str))]
    args: Vec<OsString>,
}

fn main() {
    let opts = Opt::from_args();
    println!("{:?}", opts);

    let mut lock = DotlockOptions::new()
    //.stale_age(std::time::Duration::from_secs(300))
        .tries(opts.tries)
        .pause(Duration::from_secs(opts.pause.trunc() as u64)
               + Duration::from_nanos((opts.pause.fract() * 1000000000.0) as u64))
        .create(&opts.lockfile).unwrap_or_else(|err| {
            println!("dotlock: Fatal error: {}", err);
            exit(111);
        });
    writeln!(lock, "Don't touch this!").unwrap();
    lock.sync_data().unwrap();

    let mut child = Command::new(&opts.command)
        .args(&opts.args)
        .spawn()
        .unwrap_or_else(|err| {
            println!("dotlock: Could not start program: {}", err);
            exit(1);
        });
    let result = child.wait().unwrap_or_else(|err| {
        println!("dotlock: Could not wait for exit: {}", err);
        exit(1);
    });

    lock.unlock().ok();
    exit(result.code().unwrap_or(111));
}