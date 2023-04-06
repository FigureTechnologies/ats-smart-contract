use crate::constants::PIO_HOME;
use crate::error::{provenance_error, Result};
use lazy_static::lazy_static;
use std::io::prelude::*;
use std::process::Command;

lazy_static! {
    pub(crate) static ref PIO_NODE: String = format!("{}/build/node0", *PIO_HOME);
}

/// Execute
///
/// ```bash
/// $  provenanced keys show -a node0 --home "$PIO_NODE" --keyring-backend test --testnet
/// ```
///
/// and capture the resulting address.
pub(crate) fn get_node0_address() -> Result<String> {
    let out = Command::new("provenanced")
        .arg("keys")
        .arg("show")
        .arg("-a")
        .arg("node0")
        .arg("--home")
        .arg(&*PIO_NODE)
        .arg("--keyring-backend")
        .arg("test")
        .arg("--testnet")
        .output()?;
    if !out.status.success() {
        return provenance_error("Couldn't get node0 address! Is Provenance localnet running?");
    };
    Ok(String::from_utf8(out.stdout)?.trim().to_owned())
}

/// Export the private key associated with localnet `node0`.
pub(crate) fn get_node0_private_key_bytes() -> Result<Vec<u8>> {
    let (reader, mut writer) = os_pipe::pipe()?;
    writer.write_all("y\n".as_bytes())?;
    let out = Command::new("provenanced")
        .stdin(reader)
        .arg("keys")
        .arg("export")
        .arg("--home")
        .arg(&*PIO_NODE)
        .arg("--keyring-backend")
        .arg("test")
        .arg("--testnet")
        .arg("--unsafe")
        .arg("--unarmored-hex")
        .arg("node0")
        .output()?;
    if !out.status.success() {
        return provenance_error("Couldn't export node0 key! Is Provenance localnet running?");
    };
    let hexbytes = String::from_utf8(out.stdout)?.trim().to_owned();
    Ok(hex::decode(hexbytes)?)
}
