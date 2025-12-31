use crate::utils::*;
use std::env;
use std::fs;
use std::io::{self, stdin, stdout, Write};
use std::path::Path;
use std::path::PathBuf;
use termion::event::Key;
use termion::input::TermRead;
use termion::raw::IntoRawMode;

pub struct Shell {
    pub input: String,
    pub cursor_pos: usize,
    pub last_key_was_tab: bool,
    // History tracking
    pub history: Vec<String>,
    pub history_index: usize,
    pub temp_input: Option<String>,
    last_written_index: usize, // ← Add this: tracks what's been written to file
}
impl Shell {
    pub fn new() -> Self {
        let mut shell = Shell {
            input: String::new(),
            cursor_pos: 0,
            last_key_was_tab: false,
            history: vec![],
            history_index: 0,
            temp_input: None,
            last_written_index: 0,
        };
        shell.load_history_on_startup();
        shell
    }
    fn load_history_on_startup(&mut self) {
        // Check if HISTFILE is set
        if let Ok(histfile) = env::var("HISTFILE") {
            // Try to load from HISTFILE
            let _ = self.load_history_from_file(Some(&histfile));
        } else {
            // Fall back to default ~/.shell_history
            let _ = self.load_history_from_file(None);
        }
    }
    pub fn save_history_on_exit(&mut self) -> Result<(), ErrorKind> {
        // Save to HISTFILE if set, otherwise to default
        if let Ok(histfile) = env::var("HISTFILE") {
            self.write_history_to_file(Some(&histfile))
        } else {
            self.write_history_to_file(None)
        }
    }
    pub fn history_command(&mut self, args: &[String]) -> Result<String, ErrorKind> {
        // eprintln!("\r\nDEBUG: history_command called with args: {:?}\r", args);
        //

        if args.is_empty() {
            // No args - print history
            // eprintln!("\r\nDEBUG: Printing {} history items\r", self.history.len());
            let mut output = String::new();
            for (i, cmd) in self.history.iter().enumerate() {
                output.push_str(&format!("{:5}  {}\n", i + 1, cmd));
            }
            Ok(output)
        } else if args[0] == "-r" {
            // eprintln!("\r\nDEBUG: -r flag detected\r");
            // Read history from file
            let filename = args.get(1).map(|s| s.as_str());
            // eprintln!("\r\nDEBUG: filename = {:?}\r", filename);
            self.load_history_from_file(filename)?;
            Ok(String::new())
        } else if args[0] == "-w" {
            // Write history to file
            let filename = args.get(1).map(|s| s.as_str());
            self.write_history_to_file(filename)?;
            Ok(String::new())
        } else if args[0] == "-a" {
            let filename = args.get(1).map(|s| s.as_str());
            self.append_history_to_file(filename)?;
            Ok(String::new())
        } else if let Ok(n) = args[0].parse::<usize>() {
            // Show last n commands
            let mut output = String::new();
            let start = if n >= self.history.len() {
                0
            } else {
                self.history.len() - n
            };

            for (i, cmd) in self.history.iter().enumerate().skip(start) {
                output.push_str(&format!("{:5}  {}\n", i + 1, cmd));
            }
            Ok(output)
        } else {
            Err(ErrorKind::CompleteFailure(format!(
                "history: invalid option: {}",
                args[0]
            )))
        }
    }

    fn append_history_to_file(&mut self, filename: Option<&str>) -> Result<(), ErrorKind> {
        use std::path::PathBuf;

        let history_path = if let Some(name) = filename {
            PathBuf::from(name)
        } else {
            self.get_history_file_path()
        };

        // Only append commands since last_written_index
        for cmd in &self.history[self.last_written_index..] {
            if let Err(_) = append_to_file(&history_path, cmd) {
                return Err(ErrorKind::CompleteFailure(format!(
                    "history: {}: cannot append to file",
                    history_path.display()
                )));
            }
        }

        // Update the index to reflect what's been written
        self.last_written_index = self.history.len();

        Ok(())
    }

    fn write_history_to_file(&mut self, filename: Option<&str>) -> Result<(), ErrorKind> {
        use std::fs;
        use std::path::PathBuf;

        let history_path = if let Some(name) = filename {
            PathBuf::from(name)
        } else {
            self.get_history_file_path()
        };

        let contents = self.history.join("\n");
        let contents = if contents.is_empty() {
            contents
        } else {
            format!("{}\n", contents)
        };

        match fs::write(&history_path, contents) {
            Ok(_) => {
                // Update last_written_index after full write
                self.last_written_index = self.history.len();
                Ok(())
            }
            Err(_) => Err(ErrorKind::CompleteFailure(format!(
                "history: {}: cannot write to file",
                history_path.display()
            ))),
        }
    }

    fn load_history_from_file(&mut self, filename: Option<&str>) -> Result<(), ErrorKind> {
        use std::fs;
        use std::path::PathBuf;

        let history_path = if let Some(name) = filename {
            PathBuf::from(name)
        } else {
            self.get_history_file_path()
        };

        // eprintln!(
        //     "\r\nDEBUG: Trying to read from: {}\r",
        //     history_path.display()
        // );
        // eprintln!("\r\nDEBUG: File exists: {}\r", history_path.exists());

        match fs::read_to_string(&history_path) {
            Ok(contents) => {
                // eprintln!("\r\nDEBUG: Read {} bytes\r", contents.len());
                // eprintln!("\r\nDEBUG: Contents:\r\n{}\r", contents);

                let before = self.history.len();

                for line in contents.lines() {
                    let line = line.trim();
                    if !line.is_empty() {
                        // eprintln!("\r\nDEBUG: Adding to history: {}\r", line);
                        self.history.push(line.to_string());
                    }
                }

                let after = self.history.len();
                // eprintln!("\r\nDEBUG: Added {} items to history\r", after - before);

                self.history_index = self.history.len();
                Ok(())
            }
            Err(e) => {
                eprintln!("\r\nDEBUG: Error reading file: {}\r", e);
                Err(ErrorKind::CompleteFailure(format!(
                    "history: {}: No such file or directory",
                    history_path.display()
                )))
            }
        }
    }
    fn get_history_file_path(&self) -> PathBuf {
        if let Ok(home) = env::var("HOME") {
            PathBuf::from(home).join(".shell_history")
        } else {
            PathBuf::from(".shell_history")
        }
    }

    pub fn add_to_history(&mut self, cmd: String) {
        if !cmd.is_empty() {
            self.history.push(cmd);
            self.history_index = self.history.len();
        }
    }
    pub fn history_prev(&mut self) {
        if self.history.is_empty() {
            return;
        }
        if self.history_index == self.history.len() {
            self.temp_input = Some(self.input.clone());
        }
        if self.history_index > 0 {
            self.history_index -= 1;
            self.input = self.history[self.history_index].clone();
        }
    }
    // Optional: save history on exit
    pub fn save_history_to_file(&self) -> Result<(), ErrorKind> {
        use std::fs;

        let history_path = self.get_history_file_path();
        let contents = self.history.join("\n");

        fs::write(&history_path, contents).map_err(|_| {
            ErrorKind::CompleteFailure(format!("history: cannot write {}", history_path.display()))
        })
    }
    pub fn history_next(&mut self) {
        if self.history_index < self.history.len() {
            self.history_index += 1;

            if self.history_index == self.history.len() {
                self.input = self.temp_input.take().unwrap_or_default();
            } else {
                self.input = self.history[self.history_index].clone();
            }
            self.cursor_pos = self.input.chars().count();
        }
    }

    pub fn get_completions(&self) -> Vec<String> {
        let partial = &self.input[..self.cursor_pos];
        let mut completions = Vec::new();

        // Get PATH directories
        if let Ok(path_env) = env::var("PATH") {
            for dir in path_env.split(':') {
                if let Ok(entries) = fs::read_dir(dir) {
                    for entry in entries.flatten() {
                        if let Ok(name) = entry.file_name().into_string() {
                            if name.starts_with(partial) {
                                completions.push(name);
                            }
                        }
                    }
                }
            }
        }

        // Add built-in commands
        let builtins = vec!["cd", "exit", "pwd", "echo", "export"];
        for builtin in builtins {
            if builtin.starts_with(partial) {
                completions.push(builtin.to_string());
            }
        }

        completions.sort();
        completions.dedup();
        completions
    }

    pub fn complete(&mut self) -> Option<String> {
        let completions = self.get_completions();

        match completions.len() {
            0 => None,
            1 => {
                // Single match - complete it
                self.input = completions[0].clone();
                self.cursor_pos = self.input.len();
                Some(self.input.clone())
            }
            _ => {
                // Multiple matches - find common prefix
                let common = self.find_common_prefix(&completions);
                if common.len() > self.cursor_pos {
                    self.input = common;
                    self.cursor_pos = self.input.len();
                    Some(self.input.clone())
                } else {
                    // Show all completions
                    None
                }
            }
        }
    }

    pub fn find_common_prefix(&self, completions: &[String]) -> String {
        if completions.is_empty() {
            return String::new();
        }

        let first = &completions[0];
        let mut prefix_len = first.len();

        for completion in &completions[1..] {
            let mut matching = 0;
            for (c1, c2) in first.chars().zip(completion.chars()) {
                if c1 == c2 {
                    matching += 1;
                } else {
                    break;
                }
            }
            prefix_len = prefix_len.min(matching);
        }

        first.chars().take(prefix_len).collect()
    }

    pub fn redraw_line<W: Write>(&self, stdout: &mut W) -> io::Result<()> {
        let char_count = self.input.chars().count();
        let move_back = char_count - self.cursor_pos;

        write!(stdout, "\r{}", termion::clear::CurrentLine)?;
        write!(stdout, "$ {}", self.input)?;

        // Only move cursor if we need to
        if move_back > 0 {
            write!(stdout, "{}", termion::cursor::Left(move_back as u16))?;
        }

        stdout.flush()
    }
}
