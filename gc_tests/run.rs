// Copyright (c) 2019 King's College London created by the Software Development Team
// <http://soft-dev.org/>
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0>, or the MIT license <LICENSE-MIT or
// http://opensource.org/licenses/MIT>, or the UPL-1.0 license <http://opensource.org/licenses/UPL>
// at your option. This file may not be copied, modified, or distributed except according to those
// terms.

/// This is a simple test runner for testing the output of Rust programs linked
/// with GC at run-time. Each ".rs" file in this directory will be compiled with
/// rustcgc, and linked with GC runtime. Each file must start with a rustdoc
/// comment in the following format:
///
/// ```text
///   <type>:
///     <body>
/// ```
///
/// where:
///
///   * `<type>` is one of "Compile-time error", "Run-time error", or "Run-time
///   output".
///   * `<body>` is fuzzy text which will be matched against the Rust program's
///   output literally except the following three exceptions:
///       * All leading/trailing whitespace in both `<body>` and the Rust
///         programs's output is ignored.
///       * A line consisting solely of `...` is a wildcard which skips as many
///         lines as necessary in order to match the subsequent line. It is an
///         error to have two consecutive lines consisting of `...`.
///       * A line starting with `...` but ending in non-whitespace matches the
///         end portion of that string.
use std::{
    env,
    fs::{read_dir, read_to_string},
    io::{self, Write},
    path::{Path, PathBuf},
    process::{self, Command}
};

use lazy_static::lazy_static;
use regex::{Regex, RegexBuilder};
use termcolor::{Color, ColorChoice, ColorSpec, StandardStream, WriteColor};

const RUST_EXT: &'static str = "rs";
const TESTS_DIR: &'static str = "gc_tests";
const WILDCARD: &'static str = "...";
const RUN_FILE: &'static str = "run.rs";
const TEST_OBJ_PATH: &'static str = "TEST_OBJ_PATH";

enum ExpType {
    CompileTimeError,
    RunTimeError,
    RunTimeOutput
}

lazy_static! {
    static ref EXPECTED: Regex = RegexBuilder::new(r#"^(/{3}(.*?)\n)*^"#)
        .multi_line(true)
        .dot_matches_new_line(false)
        .build()
        .unwrap();
}

fn find_tests() -> Vec<PathBuf> {
    let mut tests_path = PathBuf::new();
    tests_path.push(env!("CARGO_MANIFEST_DIR"));
    tests_path.push(TESTS_DIR);
    read_dir(&tests_path)
        .unwrap()
        .map(|x| x.unwrap().path())
        .filter(|x| x.extension().unwrap() == RUST_EXT)
        .filter(|x| x.file_name().unwrap() != RUN_FILE)
        .collect()
}

fn write_with_colour(s: &str, colour: Color) {
    let mut stderr = StandardStream::stderr(ColorChoice::Always);
    stderr.set_color(ColorSpec::new().set_fg(Some(colour))).ok();
    io::stderr().write_all(s.as_bytes()).ok();
    stderr.reset().ok();
}

fn output_failure(test_name: &str, output: process::Output) {
    write_with_colour("FAILED", Color::Red);
    eprintln!("\n---- {} stdout ----", test_name);
    io::stderr().write_all(&output.stdout).unwrap();
    eprintln!("\n---- {} stderr ----", test_name);
    io::stderr().write_all(&output.stderr).unwrap();
}

fn output_success() {
    write_with_colour("ok", Color::Green);
}

fn fuzzy_match(pattern: &str, s: &[u8]) -> bool {
    let plines = pattern
        .trim()
        .lines()
        .map(|x| x.trim_start_matches("///").trim())
        .collect::<Vec<_>>();
    let slines = std::str::from_utf8(s)
        .unwrap()
        .trim()
        .lines()
        .map(|x| x.trim())
        .collect::<Vec<_>>();

    let mut pi = 0;
    let mut si = 0;

    while pi < plines.len() && si < slines.len() {
        if plines[pi] == WILDCARD {
            pi += 1;
            if pi == plines.len() {
                return true;
            }
            if plines[pi] == WILDCARD {
                panic!("Can't have '{}' on two consecutive lines.", WILDCARD);
            }
            while si < slines.len() {
                if plines[pi] == slines[si] {
                    break;
                }
                si += 1;
            }
            if si == slines.len() {
                return false;
            }
        } else if (plines[pi].starts_with(WILDCARD)
            && slines[si].ends_with(plines[pi][WILDCARD.len()..].trim()))
            || plines[pi] == slines[si]
        {
            pi += 1;
            si += 1;
        } else {
            return false;
        }
    }
    true
}

fn run_test(path: PathBuf) -> bool {
    let test_name = path.file_name().unwrap().to_str().unwrap();
    eprint!("test lang_tests::{} ... ", test_name);
    let d = read_to_string(&path).unwrap();
    let exp = EXPECTED
        .captures(&d)
        .expect(&format!(
            "{} doesn't contain expected test output",
            test_name
        ))
        .get(0)
        .unwrap()
        .as_str()
        .trim();
    let exptype = match &*exp
        .lines()
        .nth(0)
        .unwrap()
        .trim_start_matches("///")
        .trim()
        .to_lowercase()
    {
        "compile-time error:" => ExpType::CompileTimeError,
        "run-time error:" => ExpType::RunTimeError,
        "run-time output:" => ExpType::RunTimeOutput,
        x => panic!("Unknown type '{}'", x)
    };

    let expbody = exp.lines().skip(1).collect::<Vec<_>>().join("\n");
    let bindir = env::var(TEST_OBJ_PATH).unwrap();
    let compile = Command::new("rustc")
        .args(&["--out-dir", &bindir, path.to_str().unwrap()])
        .output()
        .expect("Couldn't run command");

    let runbin = || {
        let bin = Path::new(&bindir).join(Path::new(path.file_stem().unwrap()));
        Command::new(bin).output().expect("Couldn't run bin")
    };

    match exptype {
        ExpType::CompileTimeError => {
            if compile.status.success() {
                output_failure(test_name, compile);
                false
            } else {
                output_success();
                true
            }
        }
        ExpType::RunTimeError => {
            if !compile.status.success() {
                output_failure(test_name, compile);
                false
            } else {
                let output = runbin();
                if fuzzy_match(&expbody, &output.stderr) {
                    output_success();
                    true
                } else {
                    output_failure(test_name, output);
                    false
                }
            }
        }
        ExpType::RunTimeOutput => {
            if !compile.status.success() {
                output_failure(test_name, compile);
                false
            } else {
                let output = runbin();
                if !output.status.success() {
                    output_failure(test_name, output);
                    false
                } else {
                    if fuzzy_match(&expbody, &output.stdout) {
                        output_success();
                        true
                    } else {
                        output_failure(test_name, output);
                        false
                    }
                }
            }
        }
    }
}

fn main() {
    let all_tests = find_tests();
    eprint!("\nrunning {} tests", all_tests.len());
    let mut any_failed = false;

    // Set up a tmp directory to store our test object binaries and SOs
    let bindir = env::var(TEST_OBJ_PATH).unwrap();
    if !Path::new(&bindir).exists() {
        Command::new("mkdir")
            .args(&[&bindir])
            .output()
            .expect("Couldn't build object directory");
    }

    for t in all_tests {
        eprintln!();
        if !run_test(t) {
            any_failed = true;
        }
    }

    eprintln!("\n");
    if any_failed {
        process::exit(1);
    }
}
