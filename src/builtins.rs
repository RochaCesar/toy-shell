use std::env;
use std::fs;

pub struct Builtins;

struct PartialSuccess {
    success_data: String,
    error_info: String,
}

enum ErrorKind {
    PartialSuccess(PartialSuccess),
    CompleteFailure(String),
}

enum Output {
    AppendStdErr(Vec<String>),
    AppendStdOut(Vec<String>),
    RedirectStdOut(Vec<String>),
    RedirectStdErr(Vec<String>),
    StdOut,
}

impl Builtins {
    pub fn new() -> Self {
        Builtins
    }
}
