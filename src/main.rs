use std::io::{self, BufRead, BufReader, Read, Write};

trait ServerInput: BufRead + Read {}
impl<T> ServerInput for T where T: BufRead + Read {}

type Logger = fn(String) -> Result<(), String>;

fn default_logger(_: String) -> Result<(), String> {
    return Ok(());
}

struct Server {
    reader: Box<dyn ServerInput>,
    writer: Box<dyn Write>,
    logger: Logger,
}

impl Server {
    fn new(reader: Box<dyn ServerInput>, writer: Box<dyn Write>) -> Server {
        return Server {
            reader: reader,
            writer: writer,
            logger: default_logger,
        };
    }

    fn with_stdio() -> Server {
        let reader = BufReader::new(io::stdin());
        return Server::new(Box::new(reader), Box::new(io::stdout()));
    }

    fn write(&mut self, s: String) -> io::Result<()> {
        return self.writer.write_all(s.as_bytes());
    }

    fn log(&mut self, s: String) -> Result<(), String> {
        let l = self.logger;
        return l(s);
    }

    fn handle_request(&mut self) -> Result<(), String> {
        let mut l = String::new();

        match self.reader.read_line(&mut l) {
            Ok(_) => (),
            Err(_) => return Err("Failed to read message".to_string()),
        }

        match self.write(l) {
            Ok(_) => (),
            Err(_) => return Err("Failed to write response".to_string()),
        }

        return Ok(());
    }

    fn start(&mut self) {
        loop {
            match self.handle_request() {
                Ok(_) => (),
                Err(e) => {
                    let msg = format!("Request failed: {}", e);
                    self.log(msg).unwrap();
                }
            }
        }
    }
}

fn main() {
    let mut server = Server::with_stdio();
    server.start();
}
