#[allow(unused_imports)]
use std::io::{self, Write};
use std::process;

fn main() {
    // Uncomment this block to pass the first stage
    loop {
        print!("$ ");
        io::stdout().flush().unwrap();
        let stdin = io::stdin();
        let mut input = String::new();
        stdin.read_line(&mut input).unwrap();
        let trimmed = input.as_str().trim();

        if let Some(rest) = trimmed.strip_prefix("echo ") {
            println!("{rest}");
        } else if let Some(code) = trimmed.strip_prefix("exit ") {
            process::exit(code.parse::<i32>().expect("Not a number"));
        } else if let Some(command) = trimmed.strip_prefix("type ") {
            match command {
                "echo" => println!("echo is a shell builtin"),
                "exit" => println!("exit is a shell builtin"),
                _ => println!("{}: command not found", trimmed),
            }
        } else {
            println!("{}: command not found", trimmed);
        }
    }
}
