use clap::{ArgAction, Parser};
use colored::Colorize;
use log::{info, LevelFilter};

/// Command line arguments
#[derive(Parser, Debug)]
#[command(about)]
pub struct Args {
    /// Host address of the jetstream server
    #[arg(short = 'H', long, default_value = "jetstream2.us-east.bsky.network")]
    pub host: String,
    /// Certificate to check jetstream server against
    #[arg(short = 'c', long, default_value = "/etc/ssl/certs/ISRG_Root_X1.pem")]
    pub certificate: String,
    /// Override threadpool size for async operations
    #[arg(short = 'a', long)]
    pub executors: Option<usize>,
    /// Override threadpool size for message parsing
    #[arg(short = 'l', long)]
    pub handlers: Option<usize>,
    /// Address of the database server (including port)
    #[arg(short = 'D', long, default_value = "127.0.0.1:8000")]
    pub dbhost: String,
    /// Username for the database server
    #[arg(short, long, default_value = "root")]
    pub username: String,
    /// Password for the database server
    #[arg(short, long, default_value = "root")]
    pub password: String,
    /// Debug verbosity level
    #[arg(short, action = ArgAction::Count)]
    pub verbosity: u8,
}

impl Args {
    /// Dump configuration to log
    pub fn dump(self: &Self) {
        // dump configuration
        info!("{}", "Configuration:".bold().underline().blue());
        info!("{}: {}", "Host".cyan(), self.host.green());
        info!("{}: {}", "Certificate".cyan(), self.certificate.green());
        info!(
            "{}: {}",
            "Executors".cyan(),
            self.executors.map_or_else(
                || "Not set, using CPU count".yellow(),
                |v| v.to_string().green()
            )
        );
        info!(
            "{}: {}",
            "Handlers".cyan(),
            self.handlers.map_or_else(
                || "Not set, using CPU count".yellow(),
                |v| v.to_string().green()
            )
        );
        info!(
            "{}: {}",
            "Verbosity Level".cyan(),
            self.log_level().to_string().green()
        );
    }

    /// Verbosity to log level
    pub fn log_level(self: &Self) -> LevelFilter {
        match self.verbosity {
            0 => LevelFilter::Info,
            1 => LevelFilter::Debug,
            _ => LevelFilter::Trace,
        }
    }
}

/// Parse command line arguments
pub fn parse_args() -> Args {
    Args::parse()
}
