use std::fs::{File, OpenOptions};
use std::io::Write;

pub trait Logger {
    fn info(&mut self, s: String) -> Result<(), String>;
    fn error(&mut self, s: String) -> Result<(), String>;
}

#[derive(Default)]
pub struct DefaultLogger {}

impl Logger for DefaultLogger {
    fn info(&mut self, _: String) -> Result<(), String> {
        Ok(())
    }

    fn error(&mut self, _: String) -> Result<(), String> {
        Ok(())
    }
}

pub struct FileLogger {
    file: File,
}

impl Logger for FileLogger {
    fn info(&mut self, s: String) -> Result<(), String> {
        self.logln(format!("INFO: {}", s))
    }

    fn error(&mut self, s: String) -> Result<(), String> {
        self.logln(format!("ERROR: {}", s))
    }
}

impl FileLogger {
    pub fn new(path: &'_ str) -> Result<FileLogger, String> {
        let mut options = OpenOptions::new();
        options.append(true).create(true);

        let file = match options.open(path) {
            Ok(f) => f,
            Err(_) => {
                return Err("Failed to create file logger".to_string())
            }
        };

        Ok(FileLogger { file })
    }

    fn log(&mut self, s: String) -> Result<(), String> {
        match self.file.write_all(s.as_bytes()) {
            Ok(_) => (),
            Err(_) => return Err("Failed to write log".to_string()),
        };

        match self.file.flush() {
            Ok(_) => Ok(()),
            Err(_) => Err("Failed to flush log".to_string()),
        }
    }

    fn logln(&mut self, s: String) -> Result<(), String> {
        self.log(format!("{}\n", s))
    }
}
