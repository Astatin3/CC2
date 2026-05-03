use std::fs::OpenOptions;
use std::io::{Read, Write};
use std::time::Duration;

fn main() -> anyhow::Result<()> {
    let mut dev = OpenOptions::new()
        .read(true)
        .write(true)
        .open("/dev/rpmsg1")?;

    let mut buf = [0u8; 4096];

    loop {
        let n = dev.read(&mut buf)?;
        println!("rpmsg read {n} bytes: {:02x?}", &buf[..n]);
    }
}
