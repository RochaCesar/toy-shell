use crate::builtins::*;
use std::fs::OpenOptions;
use std::io::{self, Write};
use std::path::Path;
use std::process::Command;
pub struct PartialSuccess {
    pub success_data: String,
    pub error_info: String,
}

pub enum ErrorKind {
    PartialSuccess(PartialSuccess),
    CompleteFailure(String),
}

pub enum Output {
    AppendStdErr(Vec<String>),
    AppendStdOut(Vec<String>),
    RedirectStdOut(Vec<String>),
    RedirectStdErr(Vec<String>),
    StdOut,
}

pub fn tokenize(input: &str) -> Vec<String> {
    let mut result = Vec::new();
    let mut current = String::new();
    let mut quote_char = None;
    let mut chars = input.chars().peekable();

    while let Some(c) = chars.next() {
        match (c, quote_char) {
            // Backslash outside quotes
            ('\\', None) => {
                if let Some(next_char) = chars.next() {
                    current.push(next_char);
                }
            }
            // Backslash inside double quotes
            ('\\', Some('"')) => {
                if let Some(next_char) = chars.next() {
                    match next_char {
                        '\\' | '$' | '"' | '\n' => current.push(next_char),
                        _ => {
                            current.push('\\');
                            current.push(next_char);
                        }
                    }
                }
            }
            // Quote handling
            ('\'' | '"', None) => quote_char = Some(c),
            ('"', Some('"')) | ('\'', Some('\'')) => quote_char = None,
            // Space handling
            (' ', None) => {
                if !current.is_empty() {
                    result.push(current.clone());
                    current.clear();
                }
            }
            // All other characters
            (c, _) => current.push(c),
        }
    }

    if !current.is_empty() {
        result.push(current);
    }

    result
}
pub fn process_partial_results(
    results: String,
    error_results: String,
) -> Result<String, ErrorKind> {
    match (results.is_empty(), error_results.is_empty()) {
        (false, false) => Err(ErrorKind::PartialSuccess(PartialSuccess {
            success_data: results,
            error_info: error_results,
        })),
        (true, _) => Err(ErrorKind::CompleteFailure(error_results)),
        (false, true) => Ok(results),
    }
}
pub fn execute_single_command(input: &str, stdout: &mut impl Write) -> io::Result<()> {
    let mut parts: Vec<String> = tokenize(input);
    let io_stream = if let Some(redirect_index) = parts.iter().position(|x| x == "2>>") {
        let vec2 = parts.split_off(redirect_index);
        Output::AppendStdErr(vec2)
    } else if let Some(redirect_index) = parts.iter().position(|x| x == "2>") {
        let vec2 = parts.split_off(redirect_index);
        Output::RedirectStdErr(vec2)
    } else if let Some(redirect_index) = parts.iter().position(|x| x == ">>" || x == "1>>") {
        let vec2 = parts.split_off(redirect_index);
        Output::AppendStdOut(vec2)
    } else if let Some(redirect_index) = parts.iter().position(|x| x == ">" || x == "1>") {
        let vec2 = parts.split_off(redirect_index);
        Output::RedirectStdOut(vec2)
    } else {
        Output::StdOut
    };

    let mut args = parts.iter_mut();
    let command = args.next().unwrap();

    let output: Result<String, ErrorKind> = match command.as_str() {
        "echo" => Builtins.echo(args.as_slice()),
        "cat" => Builtins.cat(args.as_slice()),
        "cd" => Builtins.cd(args.next().unwrap()),
        "pwd" => Builtins.pwd(),
        "type" => Builtins._type(args.next().map(|x| x.as_str())),
        _ => {
            if let Ok(child) = std::process::Command::new(&command)
                .args(args)
                .stdout(std::process::Stdio::piped())
                .output()
            {
                let stdout = String::from_utf8_lossy(&child.stdout).to_string();
                let stderr = String::from_utf8_lossy(&child.stderr).to_string();
                process_partial_results(stdout, stderr)
            } else {
                Err(ErrorKind::CompleteFailure(format!(
                    "{}: command not found",
                    &command
                )))
            }
        }
    };
    handle_output(output, io_stream, stdout)?;
    Ok(())
}

pub fn execute_pipeline(input: &str, stdout: &mut impl Write) -> io::Result<()> {
    let commands: Vec<&str> = input.split("|").map(|s| s.trim()).collect();

    let mut previous_stdout: Option<std::process::Stdio> = None;
    let mut processes = vec![];

    for (i, cmd) in commands.iter().enumerate() {
        let parts: Vec<String> = tokenize(cmd);
        if parts.is_empty() {
            continue;
        }
        let command = &parts[0];
        let args = &parts[1..];

        if matches!(command.as_str(), "cd" | "exit" | "pwd" | "type" | "echo") {
            writeln!(stdout, "Error: builtins in pipes not supported\r")?;
            return Ok(());
        }

        let mut cmd = std::process::Command::new(command);
        cmd.args(args);

        if let Some(prev) = previous_stdout.take() {
            cmd.stdin(prev);
        }

        if i < commands.len() - 1 {
            cmd.stdout(std::process::Stdio::piped());
        }

        match cmd.spawn() {
            Ok(mut child) => {
                previous_stdout = child.stdout.take().map(std::process::Stdio::from);
                processes.push(child);
            }
            Err(_) => {
                writeln!(stdout, "{}: command not found\r", command)?;
                return Ok(());
            }
        }
    }

    for mut process in processes {
        let _ = process.wait();
    }
    Ok(())
}

fn handle_output(
    output: Result<String, ErrorKind>,
    io_stream: Output,
    stdout: &mut impl Write,
) -> io::Result<()> {
    match io_stream {
        Output::AppendStdErr(vec2) => {
            let filename = vec2.iter().skip(1).next().unwrap();
            let path = Path::new(filename);

            match output {
                Ok(correct_output) => {
                    append_to_file(path, "").expect("Error happened");
                    println!("{}", correct_output.as_str().trim());
                }
                Err(ErrorKind::CompleteFailure(error_message)) => {
                    append_to_file(path, error_message.trim()).expect("Error happened");
                }
                Err(ErrorKind::PartialSuccess(partial_success)) => {
                    append_to_file(path, &partial_success.error_info.trim())
                        .expect("Error happened");
                    println!("{}", partial_success.success_data.trim());
                }
            }
        }
        Output::AppendStdOut(vec2) => {
            let filename = vec2.iter().skip(1).next().unwrap();
            let path = Path::new(filename);

            match output {
                Ok(correct_output) => {
                    append_to_file(path, correct_output.as_str().trim()).expect("Error happened");
                }
                Err(ErrorKind::CompleteFailure(error_message)) => {
                    if let Ok(_) = append_to_file(path, "") {}
                    println!("{}", error_message.trim());
                }
                Err(ErrorKind::PartialSuccess(partial_success)) => {
                    append_to_file(path, &partial_success.success_data).expect("Error happened");
                    println!("{}", partial_success.error_info.trim());
                }
            }
        }
        Output::RedirectStdErr(vec2) => {
            let filename = vec2.iter().skip(1).next().unwrap();

            match output {
                Ok(correct_output) => {
                    if !correct_output.is_empty() {
                        println!("{}", correct_output.trim());
                    }
                    std::fs::write(filename, "").expect("failed");
                }
                Err(ErrorKind::CompleteFailure(error_message)) => {
                    std::fs::write(filename, error_message + "\n").expect("failed");
                }
                Err(ErrorKind::PartialSuccess(partial_success)) => {
                    std::fs::write(filename, partial_success.error_info + "\n").expect("failed");
                    println!("{}", partial_success.success_data.trim());
                }
            }
        }
        Output::RedirectStdOut(vec2) => {
            let filename = vec2.iter().skip(1).next().unwrap();

            match output {
                Ok(correct_output) => {
                    std::fs::write(filename, correct_output + "\n").expect("failed")
                }
                Err(ErrorKind::CompleteFailure(error_message)) => {
                    println!("{}", error_message.trim())
                }
                Err(ErrorKind::PartialSuccess(partial_success)) => {
                    std::fs::write(filename, partial_success.success_data + "\n").expect("failed");
                    println!("{}", partial_success.error_info);
                }
            }
        }
        Output::StdOut => match output {
            Ok(correct_output) => {
                if !correct_output.is_empty() {
                    let output = correct_output.replace("\n", "\r\n");
                    write!(stdout, "{}\r\n", output.trim())?;
                    stdout.flush()?;
                }
            }
            Err(ErrorKind::CompleteFailure(error_message)) => {
                eprintln!("{}", error_message.trim())
            }
            Err(ErrorKind::PartialSuccess(partial_success)) => {
                println!("{}", partial_success.success_data.trim());
                eprintln!("{}", partial_success.error_info.trim());
            }
        },
    }
    Ok(())
}

pub fn append_to_file(path: &Path, content: &str) -> std::io::Result<()> {
    let mut file = OpenOptions::new()
        .create(true) // Create the file if it doesn't exist
        .append(true) // Open in append mode
        .open(path)?;

    if !content.is_empty() {
        writeln!(file, "{}", content)?;
    }

    Ok(())
}
