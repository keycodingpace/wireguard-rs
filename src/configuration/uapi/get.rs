use log;
use std::io;

use super::Configuration;

pub fn serialize<C: Configuration, W: io::Write>(writer: &mut W, config: &C) -> io::Result<()> {
    let mut write = |key: &'static str, value: String| {
        debug_assert!(value.is_ascii());
        debug_assert!(key.is_ascii());
        log::trace!("UAPI: return : {} = {}", key, value);
        writer.write(key.as_ref())?;
        writer.write(b"=")?;
        writer.write(value.as_ref())?;
        writer.write(b"\n")
    };

    // serialize interface
    config
        .get_private_key()
        .map(|sk| write("private_key", hex::encode(sk.to_bytes())));

    config
        .get_listen_port()
        .map(|port| write("listen_port", port.to_string()));

    config
        .get_fwmark()
        .map(|fwmark| write("fwmark", fwmark.to_string()));

    // serialize all peers
    let mut peers = config.get_peers();
    while let Some(p) = peers.pop() {
        write("rx_bytes", p.rx_bytes.to_string())?;
        write("tx_bytes", p.tx_bytes.to_string())?;
        write(
            "last_handshake_time_sec",
            p.last_handshake_time_nsec.to_string(),
        )?;
        write(
            "last_handshake_time_nsec",
            p.last_handshake_time_nsec.to_string(),
        )?;
        write("public_key", hex::encode(p.public_key.as_bytes()))?;
        write("preshared_key", hex::encode(p.preshared_key))?;
        for (ip, cidr) in p.allowed_ips {
            write("allowed_ip", ip.to_string() + "/" + &cidr.to_string())?;
        }
    }

    Ok(())
}