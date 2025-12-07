use rustyline::{DefaultEditor, error::ReadlineError};
use std::str::FromStr;
use tracing::{info, warn};

use crate::wallet::state::Wallet;

#[derive(Debug)]
enum Command {
    Balance,
    Unknown(String),
}

impl FromStr for Command {
    type Err = std::convert::Infallible;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        match input.trim().to_lowercase().as_str() {
            "balance" => Ok(Command::Balance),
            cmd => Ok(Command::Unknown(cmd.to_string())),
        }
    }
}

pub async fn start_console(wallet: Wallet) {
    tokio::task::spawn_blocking(move || {
        let mut rl = DefaultEditor::new().expect("failed to init rustyline");

        loop {
            match rl.readline("kelp> ") {
                Ok(line) => {
                    let cmd = line.trim();
                    if cmd.is_empty() {
                        continue;
                    }

                    match cmd.parse::<Command>() {
                        Ok(Command::Balance) => {
                            info!("Balance: {} NPT.", wallet.utxos.read().unwrap().summary);
                        }
                        Ok(Command::Unknown(cmd)) => {
                            warn!("Unknown command: {}", cmd);
                        }
                        Err(_) => unreachable!("Infallible error cannot occur"),
                    }
                }
                Err(ReadlineError::Interrupted) => {
                    std::process::exit(0);
                }
                Err(ReadlineError::Eof) => {
                    continue;
                }
                Err(err) => {
                    panic!("Unexpected error: {:?}", err);
                }
            }
        }
    });
}
