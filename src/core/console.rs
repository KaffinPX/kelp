use neptune_privacy::api::export::{KeyType, NativeCurrencyAmount, Network, ReceivingAddress};
use rustyline::{DefaultEditor, error::ReadlineError};
use std::str::FromStr;
use tracing::{info, warn};

use crate::wallet::flow::Wallet;

#[derive(Debug)]
enum Command {
    Height,
    Balance,
    Address,
    Send,
    Unknown(String),
}

impl FromStr for Command {
    type Err = std::convert::Infallible;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        match input.trim().to_lowercase().as_str() {
            "height" => Ok(Command::Height),
            "balance" => Ok(Command::Balance),
            "address" => Ok(Command::Address),
            "send" => Ok(Command::Send),
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
                    let mut parts_iter = line.trim().split_whitespace();
                    let cmd = match parts_iter.next() {
                        Some(s) if !s.is_empty() => s.to_lowercase(),
                        _ => continue,
                    };

                    match cmd.parse::<Command>() {
                        Ok(Command::Height) => {
                            info!("Height: {}.", wallet.scanner.height());
                        }
                        Ok(Command::Balance) => {
                            info!("Balance: {} XNT.", wallet.utxos.blocking_read().summary);
                        }
                        Ok(Command::Address) => {
                            let keys = wallet.keys.blocking_read();
                            println!(
                                "{}",
                                keys.current_key(KeyType::Generation)
                                    .to_address()
                                    .to_bech32m(Network::Main)
                                    .unwrap()
                            );
                        }
                        Ok(Command::Send) => {
                            let address_str = match parts_iter.next() {
                                Some(s) => s,
                                None => {
                                    warn!("Missing address.");
                                    continue;
                                }
                            };
                            let address =
                                match ReceivingAddress::from_bech32m(address_str, Network::Main) {
                                    Ok(addr) => addr,
                                    Err(e) => {
                                        warn!("Invalid address: {}.", e);
                                        continue;
                                    }
                                };
                            let amount_str = match parts_iter.next() {
                                Some(s) => s,
                                None => {
                                    warn!("Missing amount.");
                                    continue;
                                }
                            };
                            let amount = match NativeCurrencyAmount::coins_from_str(amount_str) {
                                Ok(a) => a,
                                Err(_) => {
                                    warn!("Invalid amount: {}.", amount_str);
                                    continue;
                                }
                            };
                            let fee_str = match parts_iter.next() {
                                Some(s) => s,
                                None => {
                                    warn!("Missing fee.");
                                    continue;
                                }
                            };
                            let fee = match NativeCurrencyAmount::coins_from_str(fee_str) {
                                Ok(f) => f,
                                Err(_) => {
                                    warn!("Invalid fee: {}.", fee_str);
                                    continue;
                                }
                            };
                            if parts_iter.next().is_some() {
                                warn!("Extra arguments for send command");
                                continue;
                            }

                            let wallet = wallet.clone();
                            tokio::runtime::Handle::current().spawn(async move {
                                wallet.transaction_builder.send(address, amount, fee).await;
                            });
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
