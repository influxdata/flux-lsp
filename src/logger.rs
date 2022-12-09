use log::{Level, LevelFilter, Metadata, Record, SetLoggerError};
use wasm_bindgen::prelude::*;
use web_sys::console;

#[wasm_bindgen]
#[derive(Debug)]
pub struct SimpleLogger;

impl log::Log for SimpleLogger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() <= Level::Info
    }

    fn log(&self, record: &Record) {
        if self.enabled(record.metadata()) {
            let data = format!("{}", record.args());
            match record.level() {
                Level::Trace => console::trace_1(&data.into()),
                Level::Debug => console::debug_1(&data.into()),
                Level::Info => console::info_1(&data.into()),
                Level::Warn => console::warn_1(&data.into()),
                Level::Error => console::error_1(&data.into()),
            }
        }
    }

    fn flush(&self) {}
}

static LOGGER: SimpleLogger = SimpleLogger;

pub fn init() -> Result<(), SetLoggerError> {
    log::set_logger(&LOGGER)
        .map(|()| log::set_max_level(LevelFilter::Info))
}
