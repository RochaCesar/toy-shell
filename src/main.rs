#[allow(unused_imports)]
use std::io::{self, Write};
use std::process;

fn main() {
    let path_env = std::env::var("PATH").unwrap();
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
            let mut paths = path_env.split(":");
            if let Some(found) =
                paths.find(|path| std::fs::metadata(format!("{path}/{command}")).is_ok())
            {
                println!("{command} is in {found}/{command}")
            } else {
                println!("{command}: not found")
            }
        } else {
            println!("{}: command not found", trimmed);
        }
    }
}
