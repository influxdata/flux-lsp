// Stdlib
use std::io::{self, BufRead, BufReader, Read, Write};

mod handler;
pub mod loggers;
mod structs;
mod utils;

use handler::Handler;
use loggers::{DefaultLogger, Logger};

pub trait ServerInput: BufRead + Read {}
impl<T> ServerInput for T where T: BufRead + Read {}

pub struct Server {
    reader: Box<dyn ServerInput>,
    writer: Box<dyn Write>,
    pub handler: Handler,
    pub logger: Box<dyn Logger>,
}

impl Server {
    pub fn new(reader: Box<dyn ServerInput>, writer: Box<dyn Write>) -> Server {
        let logger = Box::new(DefaultLogger {});
        let server = Server {
            reader: reader,
            writer: writer,
            logger: logger,
            handler: Handler::new(),
        };

        return server;
    }

    pub fn with_stdio() -> Server {
        let reader = BufReader::new(io::stdin());
        return Server::new(Box::new(reader), Box::new(io::stdout()));
    }

    fn write(&mut self, s: String) -> io::Result<()> {
        let st = s.clone();
        let result = st.as_bytes();
        let size = result.len();
        let full = String::from(format!("Content-Length: {}\r\n\r\n{}", size, s));
        let data = Vec::from(full.as_bytes());

        self.writer.write_all(&data)?;
        return self.writer.flush();
    }

    fn read_spacer(&mut self) -> Result<(), String> {
        let mut s = String::new();
        match &self.reader.read_line(&mut s) {
            Ok(_) => return Ok(()),
            Err(_) => return Err("Failed to read spacer line".to_string()),
        }
    }

    fn read_content_body(&mut self, size: usize) -> Result<String, String> {
        self.read_spacer()?;
        let mut vec = vec![0; size];
        match self.reader.read_exact(&mut vec) {
            Ok(_) => (),
            Err(_) => return Err("Failed to get input".to_string()),
        }

        match std::str::from_utf8(&vec) {
            Ok(contents) => return Ok(contents.to_string()),
            Err(_) => return Err("Failed to parse contents".to_string()),
        }
    }

    fn handle_request(&mut self) -> Result<(), String> {
        let mut line = String::new();

        match self.reader.read_line(&mut line) {
            Ok(_) => (),
            Err(_) => return Err("Failed to read message".to_string()),
        }

        if !line.starts_with("Content-Length") {
            return Err("Malformed request\n".to_string());
        }

        let content_size = utils::get_content_size(line.clone())?;
        self.logger
            .info(format!("Request Content Size: {}", content_size))?;

        let content_body = self.read_content_body(content_size)?;
        self.logger
            .info(format!("Request Content Body: {}", content_body))?;

        let request = utils::parse_request(content_body)?;
        let msg = self.handler.handle(request)?;

        if msg != String::from("") {
            match self.write(msg) {
                Ok(_) => return Ok(()),
                Err(_) => return Err("Failed to write response".to_string()),
            }
        }

        return Ok(());
    }

    pub fn start(&mut self) {
        loop {
            match self.handle_request() {
                Ok(_) => (),
                Err(e) => {
                    let msg = format!("Request failed: {}\n", e);
                    self.logger.error(msg).unwrap();
                }
            }
        }
    }
}
