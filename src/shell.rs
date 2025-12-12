use std::env;
use std::fs;
use std::io::{self, stdin, stdout, Write};
use std::path::Path;
use termion::event::Key;
use termion::input::TermRead;
use termion::raw::IntoRawMode;

pub struct Shell {
    pub input: String,
    pub cursor_pos: usize,
}
impl Shell {
    pub fn new() -> Self {
        Shell {
            input: String::new(),
            cursor_pos: 0,
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
        write!(
            stdout,
            "\r{}$ {}{}",
            termion::clear::CurrentLine,
            self.input,
            termion::cursor::Left((self.input.len() - self.cursor_pos) as u16)
        )?;
        stdout.flush()
    }
}
