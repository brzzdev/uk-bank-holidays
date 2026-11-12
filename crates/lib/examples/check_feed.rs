//! Checks that a file parses as the gov.uk bank holidays feed: `check_feed FILE`.

use std::process::ExitCode;
use std::{env, fs};

use bank_holidays::Feed;

fn main() -> ExitCode {
    let Some(path) = env::args_os().nth(1) else {
        eprintln!("usage: check_feed FILE");
        return ExitCode::from(2);
    };
    let bytes = match fs::read(&path) {
        Ok(bytes) => bytes,
        Err(error) => {
            eprintln!("check_feed: couldn't read {}: {error}", path.display());
            return ExitCode::FAILURE;
        }
    };
    if let Err(error) = Feed::parse(&bytes) {
        eprintln!("check_feed: {error}");
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}
