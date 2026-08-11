use std::env;
use std::io;
use std::net::SocketAddr;
use std::process::ExitCode;

use dctp_sim::SimulatorServer;

const DEFAULT_LISTEN_ADDRESS: &str = "127.0.0.1:7100";

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("dctp-sim: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> io::Result<()> {
    let address = parse_listen_address(env::args().skip(1))?;
    SimulatorServer::spawn(address)?.run_forever()
}

fn parse_listen_address(args: impl Iterator<Item = String>) -> io::Result<SocketAddr> {
    let mut address = DEFAULT_LISTEN_ADDRESS.to_owned();
    let mut args = args.peekable();
    while let Some(argument) = args.next() {
        match argument.as_str() {
            "--listen" => {
                address = args.next().ok_or_else(|| {
                    io::Error::new(io::ErrorKind::InvalidInput, "--listen requires an address")
                })?;
            }
            "--help" | "-h" => {
                println!("Usage: dctp-sim [--listen ADDRESS]");
                std::process::exit(0);
            }
            _ => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("unknown argument: {argument}"),
                ));
            }
        }
    }
    address.parse().map_err(|error| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("invalid listen address {address:?}: {error}"),
        )
    })
}
