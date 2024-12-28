#[allow(unused_imports)]
use std::io::{self, Write};

fn main() {
    // Uncomment this block to pass the first stage
    loop {
        print!("$ ");
        io::stdout().flush().unwrap();
        let stdin = io::stdin();
        let mut input = String::new();
        stdin.read_line(&mut input).unwrap();
        match input.as_str().trim() {
            "exit 0" => break,
            trimmed => match trimmed.strip_prefix("echo ") {
                Some(rest) => println!("{rest}"),
                _ => println!("{}: command not found", trimmed),
            },
        }
    }
}
