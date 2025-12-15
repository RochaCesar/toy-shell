use crate::utils::*;
use std::env;
use std::fs;
use std::fs::OpenOptions;
use std::io::prelude::*;
use std::io::{self, Write};
use std::path::Path;

pub struct Builtins;

impl Builtins {
    pub fn new() -> Self {
        Builtins
    }
    pub fn echo(&self, args: &[String]) -> Result<String, ErrorKind> {
        Ok(args.join(" "))
    }
    pub fn cat(&self, args: &[String]) -> Result<String, ErrorKind> {
        use std::fs::File;
        use std::path::Path;

        let mut result = vec![];
        let mut error_result = vec![];
        for file_name in args {
            let path = Path::new(&file_name);
            let display = path.display();

            if let Ok(mut file) = File::open(&path) {
                let mut s = String::new();

                match file.read_to_string(&mut s) {
                    Err(why) => panic!("couldn't read {}: {}", display, why),
                    Ok(_) => result.push(s),
                }
            } else {
                error_result.push(format!("cat: {}: No such file or directory", file_name));
            }
        }
        process_partial_results(result.concat(), error_result.concat())
    }
    pub fn cd(&self, path: &str) -> Result<String, ErrorKind> {
        let home_env = std::env::var("HOME").unwrap();
        if path == "" || path == " " || path == "~" {
            // Go home on empty path
            let target = Path::new(&home_env);
            match std::env::set_current_dir(&target) {
                Err(_) => Err(ErrorKind::CompleteFailure(format!(
                    "cd: {}: No such file or directory",
                    target.display()
                ))),
                Ok(_) => Ok(String::new()),
            }
        } else if &path[0..1] == "/" {
            // Handle absolute paths
            let target = Path::new(&path);

            match std::env::set_current_dir(&target) {
                Err(_) => Err(ErrorKind::CompleteFailure(format!(
                    "cd: {}: No such file or directory",
                    target.display()
                ))),
                _ => Ok(String::new()),
            }
        } else {
            let get_current_directory = std::env::current_dir().expect("Invalid Directory");
            let current_directory = get_current_directory
                .to_str()
                .expect("Error converting to string");
            let target_directory = current_directory
                .chars()
                .chain(std::iter::once('/'))
                .chain(path.chars())
                .collect::<String>();
            let mut destination = vec![];
            for directory in target_directory.split("/") {
                match directory {
                    "." => {}
                    ".." => {
                        destination.pop().unwrap();
                    }
                    _ => {
                        destination.push(directory);
                    }
                }
            }
            let final_destination = destination.join("/");

            let target = Path::new(&final_destination);

            match std::env::set_current_dir(&target) {
                Err(_) => Err(ErrorKind::CompleteFailure(format!(
                    "cd: {}: No such file or directory",
                    target.display()
                ))),
                _ => Ok(String::new()),
            }
        }
    }
    pub fn pwd(&self) -> Result<String, ErrorKind> {
        Ok(format!(
            "{}",
            std::env::current_dir()
                .expect("Invalid Directory")
                .display()
        ))
    }
    pub fn _type(&self, path: Option<&str>) -> Result<String, ErrorKind> {
        if let Some(argument) = path {
            if argument == "cd"
                || argument == "echo"
                || argument == "exit"
                || argument == "type"
                || argument == "pwd"
            // || argument == "cat"
            {
                Ok(format!("{argument} is a shell builtin"))
            } else if let Some(found) =
                std::env::var("PATH")
                    .unwrap_or_default()
                    .split(":")
                    .find(|path| {
                        let file_path = format!("{path}/{argument}");
                        if let Ok(metadata) = std::fs::metadata(&file_path) {
                            #[cfg(unix)]
                            {
                                use std::os::unix::fs::PermissionsExt;
                                // Check if executable bit is set (0o111 = any execute permission)
                                metadata.permissions().mode() & 0o111 != 0
                            }
                        } else {
                            false
                        }
                    })
            {
                Ok(format!("{argument} is {found}/{argument}"))
            } else {
                Err(ErrorKind::CompleteFailure(format!("{argument}: not found")))
            }
        } else {
            // TODO
            Ok(String::new())
        }
    }
}
