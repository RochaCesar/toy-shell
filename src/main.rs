#[allow(unused_imports)]
use std::io::{self, Write};
use std::process;

fn main() {
    let path_env = std::env::var("PATH").unwrap();
    loop {
        print!("$ ");
        io::stdout().flush().unwrap();
        let stdin = io::stdin();
        let mut input = String::new();
        stdin.read_line(&mut input).unwrap();
        let trimmed = input.as_str().trim();

        if let Some(rest) = trimmed.strip_prefix("echo ") {
            println!("{rest}");
        } else if let Some(_) = trimmed.strip_prefix("pwd") {
            println!(
                "{}",
                std::env::current_dir()
                    .expect("Invalid Directory")
                    .display()
            );
        } else if let Some(code) = trimmed.strip_prefix("exit ") {
            process::exit(code.parse::<i32>().expect("Not a number"));
        } else if let Some(command) = trimmed.strip_prefix("type ") {
            let mut paths = path_env.split(":");
            if command == "echo" || command == "exit" || command == "type" || command == "pwd" {
                println!("{command} is a shell builtin")
            } else if let Some(found) =
                paths.find(|path| std::fs::metadata(format!("{path}/{command}")).is_ok())
            {
                println!("{command} is {found}/{command}")
            } else {
                println!("{command}: not found")
            }
        } else {
            let mut parts = input.trim().split_whitespace();
            let command = parts.next().unwrap();
            let args = parts;

            if let Ok(mut child) = std::process::Command::new(command).args(args).spawn() {
                let _ = child.wait();
            } else {
                println!("{command}: command not found");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_name() {
        let testing: &str = "exit 100";

        match testing.as_bytes() {
            [b'e', b'x', b'i', b't', b' ', rest @ ..] => todo!(),
            _ => {}
        }

        assert!(false);
    }
}
