#[macro_use]
extern crate lazy_static;

// Stdlib
use std::cell::RefCell;
use std::io::{self, BufRead, BufReader, Read, Write};
use std::rc::Rc;

pub mod handler;
pub mod handlers;
pub mod loggers;
pub mod protocol;
pub mod utils;

mod cache;
mod visitors;

use handler::Handler;
use loggers::Logger;

pub trait ServerInput: BufRead + Read {}
impl<T> ServerInput for T where T: BufRead + Read {}

pub struct Server {
    reader: Box<dyn ServerInput>,
    writer: Box<dyn Write>,
    pub handler: Handler,
    pub logger: Rc<RefCell<dyn Logger>>,
}

impl Server {
    pub fn new(
        logger: Rc<RefCell<dyn Logger>>,
        reader: Box<dyn ServerInput>,
        writer: Box<dyn Write>,
        disable_folding: bool,
    ) -> Server {
        Server {
            reader,
            writer,
            logger: logger.clone(),
            handler: Handler::new(logger.clone(), disable_folding),
        }
    }

    pub fn with_stdio(
        logger: Rc<RefCell<dyn Logger>>,
        disable_folding: bool,
    ) -> Server {
        let reader = BufReader::new(io::stdin());
        Server::new(
            logger,
            Box::new(reader),
            Box::new(io::stdout()),
            disable_folding,
        )
    }

    fn write(&mut self, s: String) -> io::Result<()> {
        let _logger = self.logger.borrow_mut();
        let st = s.clone();
        let result = st.as_bytes();
        let size = result.len();
        let full = format!("Content-Length: {}\r\n\r\n{}", size, s);
        let data = Vec::from(full.as_bytes());

        self.writer.write_all(&data)?;
        self.writer.flush()
    }

    fn read_spacer(&mut self) -> Result<(), String> {
        let _logger = self.logger.borrow_mut();
        let mut s = String::new();
        match &self.reader.read_line(&mut s) {
            Ok(_) => Ok(()),
            Err(_) => Err("Failed to read spacer line".to_string()),
        }
    }

    fn read_content_body(
        &mut self,
        size: usize,
    ) -> Result<String, String> {
        self.read_spacer()?;
        let mut vec = vec![0; size];
        match self.reader.read_exact(&mut vec) {
            Ok(_) => (),
            Err(_) => return Err("Failed to get input".to_string()),
        }

        match std::str::from_utf8(&vec) {
            Ok(contents) => Ok(contents.to_string()),
            Err(_) => Err("Failed to parse contents".to_string()),
        }
    }

    fn handle_request(&mut self) -> Result<(), String> {
        let mut line = String::new();

        match self.reader.read_line(&mut line) {
            Ok(_) => (),
            Err(_) => {
                return Err("Failed to read message".to_string())
            }
        }

        if !line.starts_with("Content-Length") {
            return Err("Malformed request\n".to_string());
        }

        let content_size = utils::get_content_size(line.clone())?;
        let content_body = self.read_content_body(content_size)?;
        let request = utils::parse_request(content_body)?;
        let option = self.handler.handle(request.clone())?;

        if let Some(msg) = option {
            match self.write(msg) {
                Ok(_) => return Ok(()),
                Err(_) => {
                    return Err("Failed to write response".to_string())
                }
            }
        }

        if request.method() == "exit" {
            std::process::exit(0);
        }

        Ok(())
    }

    pub fn start(&mut self) {
        loop {
            match self.handle_request() {
                Ok(_) => (),
                Err(e) => {
                    let mut logger = self.logger.borrow_mut();
                    let msg = format!("Request failed: {}\n", e);
                    logger.error(msg).unwrap();
                }
            }
        }
    }
}
