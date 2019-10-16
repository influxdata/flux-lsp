// Stdlib
use std::io::{self, BufRead, BufReader, Read, Write};

mod handler;
mod loggers;
mod structs;
mod utils;

use handler::Handler;
use loggers::{DefaultLogger, FileLogger, Logger};

trait ServerInput: BufRead + Read {}
impl<T> ServerInput for T where T: BufRead + Read {}

struct Server {
    reader: Box<dyn ServerInput>,
    writer: Box<dyn Write>,
    handler: Handler,
    logger: Box<dyn Logger>,
}

impl Server {
    fn new(reader: Box<dyn ServerInput>, writer: Box<dyn Write>) -> Server {
        let logger = Box::new(DefaultLogger {});
        let server = Server {
            reader: reader,
            writer: writer,
            logger: logger,
            handler: Handler::new(),
        };

        return server;
    }

    fn with_stdio() -> Server {
        let reader = BufReader::new(io::stdin());
        return Server::new(Box::new(reader), Box::new(io::stdout()));
    }

    fn write(&mut self, s: String) -> io::Result<()> {
        let st = s.clone();
        let result = st.as_bytes();
        let size = result.len();
        let full = String::from(format!("Content-Length: {}\r\n\r\n{}", size, s));
        let data = Vec::from(full.as_bytes());

        match self.log(full) {
            Ok(_) => (()),
            Err(_) => (()),
        }

        self.writer.write_all(&data)?;
        return self.writer.flush();
    }

    fn log(&mut self, s: String) -> Result<(), String> {
        return self.logger.log(s.clone());
    }

    fn read_spacer(&mut self) -> Result<(), String> {
        let mut s = String::new();
        match &self.reader.read_line(&mut s) {
            Ok(_) => return Ok(()),
            Err(_) => return Err("Failed to read spacer line".to_string()),
        }
    }

    fn read_contents(&mut self, size: usize) -> Result<Vec<u8>, String> {
        let mut vec = vec![0; size];

        match self.reader.read_exact(&mut vec) {
            Ok(_) => return Ok(vec),
            Err(_) => return Err("Failed to get input".to_string()),
        }
    }

    fn read_content_body(&mut self, size: usize) -> Result<String, String> {
        self.read_spacer()?;
        let vec = self.read_contents(size)?;

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
        self.log(format!(
            "\n------\nContent Size: {}\n------\n",
            content_size
        ))?;

        let content_body = self.read_content_body(content_size)?;
        self.log(format!("\n------\n{}\n------\n", content_body))?;

        let request = utils::parse_request(content_body)?;
        let msg = self.handler.handle(request)?;

        match self.write(msg) {
            Ok(_) => return Ok(()),
            Err(_) => return Err("Failed to write response".to_string()),
        }
    }

    fn start(&mut self) {
        loop {
            match self.handle_request() {
                Ok(_) => (),
                Err(e) => {
                    let msg = format!("Request failed: {}\n", e);
                    self.log(msg).unwrap();
                }
            }
        }
    }
}

fn main() {
    let server_log = FileLogger::new("lsp.log").unwrap();
    let handler_log = FileLogger::new("lsp.log").unwrap();

    let mut server = Server::with_stdio();
    server.logger = Box::new(server_log);
    server.handler.set_logger(Box::new(handler_log));

    server.start();
}
