use std::fs::{File, OpenOptions};
use std::io::{self, Write};

pub trait Logger {
    fn log(&mut self, s: String) -> Result<(), String>;
}

pub struct DefaultLogger {}

impl Logger for DefaultLogger {
    fn log(&mut self, s: String) -> Result<(), String> {
        match io::stdout().write_all(s.as_bytes()) {
            Ok(_) => return Ok(()),
            Err(_) => return Err("Failed to write log".to_string()),
        }
    }
}

pub struct FileLogger {
    file: File,
}

impl Logger for FileLogger {
    fn log(&mut self, s: String) -> Result<(), String> {
        match self.file.write_all(s.as_bytes()) {
            Ok(_) => return Ok(()),
            Err(_) => return Err("Failed to write log".to_string()),
        }
    }
}

impl FileLogger {
    pub fn new(path: &'static str) -> Result<FileLogger, String> {
        let mut options = OpenOptions::new();
        options.append(true).create(true);

        let file = match options.open(path) {
            Ok(f) => f,
            Err(_) => return Err("Failed to create file logger".to_string()),
        };

        return Ok(FileLogger { file: file });
    }
}
