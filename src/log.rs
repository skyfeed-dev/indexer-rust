use chrono::Local;
use colog::{format::CologStyle, formatter};
use colored::Colorize;
use log::{Level, LevelFilter};

/// Custom log style for colog
struct LogStyle;

impl CologStyle for LogStyle {
    fn prefix_token(&self, level: &Level) -> String {
        // convert log level to colored string
        let prefix = match level {
            Level::Error => "E".red(),
            Level::Warn => "W".yellow(),
            Level::Info => "*".green(),
            Level::Debug => "D".blue(),
            Level::Trace => "T".purple(),
        };

        // format current time
        let time = Local::now().format("%d.%m.%Y %H:%M:%S");

        // return formatted log prefix
        format!("[{}] [{}]", time, prefix)
    }
}

/// Initialize the logging system
pub fn init(level: LevelFilter) {
    colog::default_builder()
        .filter_level(LevelFilter::Off)
        .filter_module("indexer", level)
        .format(formatter(LogStyle))
        .init();
}
