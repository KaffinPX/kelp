use neptune_privacy::api::export::{KeyType, Network};
use rustyline::{DefaultEditor, error::ReadlineError};
use std::str::FromStr;
use tracing::{info, warn};

use crate::wallet::state::Wallet;

#[derive(Debug)]
enum Command {
    Height,
    Balance,
    Address,
    Unknown(String),
}

impl FromStr for Command {
    type Err = std::convert::Infallible;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        match input.trim().to_lowercase().as_str() {
            "height" => Ok(Command::Height),
            "balance" => Ok(Command::Balance),
            "address" => Ok(Command::Address),
            cmd => Ok(Command::Unknown(cmd.to_string())),
        }
    }
}

pub async fn start_console(wallet: Wallet) {
    tokio::task::spawn_blocking(move || {
        let mut rl = DefaultEditor::new().expect("failed to init rustyline");

        loop {
            match rl.readline("") {
                Ok(line) => {
                    let cmd = line.trim();
                    if cmd.is_empty() {
                        continue;
                    }

                    match cmd.parse::<Command>() {
                        Ok(Command::Height) => {
                            info!("Height: {}.", wallet.height.read().unwrap());
                        }
                        Ok(Command::Balance) => {
                            info!("Balance: {} XNT.", wallet.utxos.read().unwrap().summary);
                        }
                        Ok(Command::Address) => {
                            let keys = wallet.keys.read().unwrap();
                            println!(
                                "{}",
                                keys.current_key(KeyType::Generation)
                                    .to_address()
                                    .to_bech32m(Network::Main)
                                    .unwrap()
                            );
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
