use dicar_firmware_flash::tool::{
    execute_package, execute_provision_record, parse_args, ToolCommand, ToolError,
};

fn main() {
    let args = std::env::args_os().skip(1).collect::<Vec<_>>();
    if let Some(help) = requested_help(&args) {
        println!("{help}");
        return;
    }
    let result = parse_args(args).and_then(|command| match command {
        ToolCommand::Package { .. } => execute_package(&command),
        ToolCommand::ProvisionRecord { .. } => execute_provision(&command),
    });
    match result {
        Ok(()) => println!("firmware package created"),
        Err(error) => {
            eprintln!("error: {error}");
            std::process::exit(2);
        }
    }
}

fn requested_help(args: &[std::ffi::OsString]) -> Option<&'static str> {
    match args {
        [argument] if argument == "--help" || argument == "-h" => Some(TOP_LEVEL_HELP),
        [command, argument]
            if command == "package" && (argument == "--help" || argument == "-h") =>
        {
            Some(PACKAGE_HELP)
        }
        [command, argument]
            if command == "provision-record" && (argument == "--help" || argument == "-h") =>
        {
            Some(PROVISION_HELP)
        }
        _ => None,
    }
}

const TOP_LEVEL_HELP: &str = "\
DiCar offline firmware package and provisioning tool

USAGE:
  dicar-firmware-tool package [OPTIONS]
  dicar-firmware-tool provision-record [OPTIONS]

Run a command with --help for its exact options.";

const PACKAGE_HELP: &str = "\
Create a signed .dicarfw package. Existing output files are never overwritten.

USAGE:
  dicar-firmware-tool package --release-id <UUID> --version <MAJOR.MINOR.PATCH> \\
    --signing-key-id <16_LOWER_HEX> --image <BIN> --key <PRIVATE_KEY> \\
    --output <DICARFW>";

const PROVISION_HELP: &str = "\
Import one device's release public key and signed recovery package.
The 32-byte raw or 64-character lower-hex BSL password is read only from stdin.

USAGE:
  dicar-firmware-tool provision-record --device-id <32_LOWER_HEX> \\
    --signing-key-id <16_LOWER_HEX> --public-key <PUBLIC_KEY> \\
    --recovery-package <DICARFW> --store-dir <DIRECTORY>";

#[cfg(windows)]
fn execute_provision(command: &ToolCommand) -> Result<(), ToolError> {
    let credentials = dicar_firmware_flash::credentials::WindowsCredentialStore;
    execute_provision_record(command, std::io::stdin().lock(), &credentials)
}

#[cfg(not(windows))]
fn execute_provision(_command: &ToolCommand) -> Result<(), ToolError> {
    Err(ToolError::Credential(
        dicar_firmware_flash::credentials::CredentialError::BackendFailure,
    ))
}
