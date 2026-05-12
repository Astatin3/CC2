use std::fs::OpenOptions;
use std::io::{ErrorKind, Read, Write};
use std::thread;
use std::time::{Duration, Instant};

const DEFAULT_DEVICE: &str = "/dev/rpmsg_ctrl-dsp_rproc@0";
const DEFAULT_COUNT: u8 = 40;
const SYNC: u8 = 0x7e;

fn main() -> anyhow::Result<()> {
    let config = Config::from_args()?;

    println!("device={}", config.device);
    println!("identify offset={} count={}", config.offset, config.count);

    let mut dev = OpenOptions::new()
        .read(true)
        .write(true)
        .open(&config.device)?;

    drain_stale_frames(&mut dev, config.drain_ms)?;

    if config.try_all_seqs {
        for seq_low in 0..=0x0f {
            let seq = 0x10 | seq_low;
            let request = encode_identify_request(seq, config.offset, config.count)?;
            println!("tx seq=0x{seq:02x} {}", hex_bytes(&request));
            dev.write_all(&request)?;
            dev.flush()?;
            thread::sleep(Duration::from_millis(config.between_writes_ms));
        }
    } else {
        let request = encode_identify_request(config.seq, config.offset, config.count)?;
        println!("tx seq=0x{:02x} {}", config.seq, hex_bytes(&request));
        dev.write_all(&request)?;
        dev.flush()?;
    }

    let mut raw = Vec::new();
    let mut buf = [0u8; 4096];
    let deadline = Instant::now() + Duration::from_millis(config.timeout_ms);
    let mut saw_identify = false;

    while Instant::now() < deadline {
        match dev.read(&mut buf) {
            Ok(0) => thread::sleep(Duration::from_millis(20)),
            Ok(n) => {
                println!("rx_chunk={}", hex_bytes(&buf[..n]));
                let start = raw.len();
                raw.extend_from_slice(&buf[..n]);
                for frame in split_frames(&raw[start..]) {
                    saw_identify |= print_frame(frame);
                }
            }
            Err(err)
                if matches!(
                    err.kind(),
                    ErrorKind::WouldBlock | ErrorKind::Interrupted | ErrorKind::TimedOut
                ) =>
            {
                thread::sleep(Duration::from_millis(20));
            }
            Err(err) => return Err(err.into()),
        }
    }

    if raw.is_empty() {
        println!("no response before timeout");
    } else {
        println!("rx_total={}", hex_bytes(&raw));
    }

    if !saw_identify {
        println!("no identify_response frame observed");
    }

    Ok(())
}

#[derive(Debug)]
struct Config {
    device: String,
    seq: u8,
    offset: u32,
    count: u8,
    timeout_ms: u64,
    drain_ms: u64,
    between_writes_ms: u64,
    try_all_seqs: bool,
}

impl Config {
    fn from_args() -> anyhow::Result<Self> {
        let mut device = DEFAULT_DEVICE.to_string();
        let mut seq = 0x10;
        let mut offset = 0;
        let mut count = DEFAULT_COUNT;
        let mut timeout_ms = 2_000;
        let mut drain_ms = 500;
        let mut between_writes_ms = 50;
        let mut try_all_seqs = false;
        let mut args = std::env::args().skip(1);

        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--device" | "-d" => {
                    device = args
                        .next()
                        .ok_or_else(|| anyhow::anyhow!("missing value for {arg}"))?;
                }
                "--offset" | "-o" => {
                    offset = args
                        .next()
                        .ok_or_else(|| anyhow::anyhow!("missing value for {arg}"))?
                        .parse()?;
                }
                "--seq" | "-s" => {
                    let value = args
                        .next()
                        .ok_or_else(|| anyhow::anyhow!("missing value for {arg}"))?;
                    seq = parse_u8(&value)?;
                }
                "--count" | "-c" => {
                    count = args
                        .next()
                        .ok_or_else(|| anyhow::anyhow!("missing value for {arg}"))?
                        .parse()?;
                }
                "--timeout-ms" | "-t" => {
                    timeout_ms = args
                        .next()
                        .ok_or_else(|| anyhow::anyhow!("missing value for {arg}"))?
                        .parse()?;
                }
                "--drain-ms" => {
                    drain_ms = args
                        .next()
                        .ok_or_else(|| anyhow::anyhow!("missing value for {arg}"))?
                        .parse()?;
                }
                "--between-writes-ms" => {
                    between_writes_ms = args
                        .next()
                        .ok_or_else(|| anyhow::anyhow!("missing value for {arg}"))?
                        .parse()?;
                }
                "--try-all-seqs" => {
                    try_all_seqs = true;
                }
                "--help" | "-h" => {
                    print_help();
                    std::process::exit(0);
                }
                _ => anyhow::bail!("unknown argument: {arg}"),
            }
        }

        Ok(Self {
            device,
            seq,
            offset,
            count,
            timeout_ms,
            drain_ms,
            between_writes_ms,
            try_all_seqs,
        })
    }
}

fn print_help() {
    println!(
        "Usage: klippy-test [--device /dev/rpmsg1] [--seq 0x10] [--offset 0] [--count 40] [--timeout-ms 2000] [--try-all-seqs]\n\n\
         Drains stale traffic, sends Klipper identify request(s), and prints raw/decoded responses.\n\
         Stop elegoo_printer before using this to avoid stealing/desyncing messages."
    );
}

fn parse_u8(value: &str) -> anyhow::Result<u8> {
    if let Some(hex) = value.strip_prefix("0x") {
        Ok(u8::from_str_radix(hex, 16)?)
    } else {
        Ok(value.parse()?)
    }
}

fn drain_stale_frames(dev: &mut std::fs::File, drain_ms: u64) -> anyhow::Result<()> {
    if drain_ms == 0 {
        return Ok(());
    }

    println!("draining stale traffic for {drain_ms}ms");
    let deadline = Instant::now() + Duration::from_millis(drain_ms);
    let mut buf = [0u8; 4096];

    while Instant::now() < deadline {
        match dev.read(&mut buf) {
            Ok(0) => thread::sleep(Duration::from_millis(20)),
            Ok(n) => println!("drain_chunk={}", hex_bytes(&buf[..n])),
            Err(err)
                if matches!(
                    err.kind(),
                    ErrorKind::WouldBlock | ErrorKind::Interrupted | ErrorKind::TimedOut
                ) =>
            {
                thread::sleep(Duration::from_millis(20));
            }
            Err(err) => return Err(err.into()),
        }
    }

    Ok(())
}

fn encode_identify_request(seq: u8, offset: u32, count: u8) -> anyhow::Result<Vec<u8>> {
    let mut content = Vec::new();
    content.push(0x01); // Klipper default: identify offset=%u count=%c
    encode_vlq(offset, &mut content)?;
    content.push(count);
    Ok(encode_frame(seq, &content))
}

fn encode_frame(seq: u8, content: &[u8]) -> Vec<u8> {
    let len = 1 + 1 + content.len() + 2 + 1;
    let mut frame = Vec::with_capacity(len);
    frame.push(len as u8);
    frame.push(seq);
    frame.extend_from_slice(content);
    let crc = crc16_ccitt_klipper(&frame);
    frame.extend_from_slice(&crc.to_be_bytes());
    frame.push(SYNC);
    frame
}

fn encode_vlq(mut value: u32, out: &mut Vec<u8>) -> anyhow::Result<()> {
    if value == 0 {
        out.push(0);
        return Ok(());
    }

    let mut bytes = [0u8; 5];
    let mut count = 0;
    while value != 0 {
        bytes[count] = (value & 0x7f) as u8;
        value >>= 7;
        count += 1;
    }

    for index in (0..count).rev() {
        let mut byte = bytes[index];
        if index != 0 {
            byte |= 0x80;
        }
        out.push(byte);
    }

    Ok(())
}

fn decode_vlq(bytes: &[u8], offset: &mut usize) -> Option<u32> {
    let mut value = 0u32;

    loop {
        let byte = *bytes.get(*offset)?;
        *offset += 1;
        value = (value << 7) | u32::from(byte & 0x7f);
        if byte & 0x80 == 0 {
            return Some(value);
        }
    }
}

fn split_frames(bytes: &[u8]) -> Vec<&[u8]> {
    let mut frames = Vec::new();
    let mut offset = 0;

    while offset < bytes.len() {
        let len = bytes[offset] as usize;
        if len == 0 || offset + len > bytes.len() {
            break;
        }
        frames.push(&bytes[offset..offset + len]);
        offset += len;
    }

    frames
}

fn print_frame(frame: &[u8]) -> bool {
    if frame.len() < 5 || frame.first().copied() != Some(frame.len() as u8) {
        println!("frame malformed raw={}", hex_bytes(frame));
        return false;
    }

    let len = frame[0];
    let seq = frame[1];
    let content = &frame[2..frame.len() - 3];
    let crc = &frame[frame.len() - 3..frame.len() - 1];
    let computed_crc = crc16_ccitt_klipper(&frame[..frame.len() - 3]).to_be_bytes();
    let term = frame[frame.len() - 1];

    println!(
        "frame len={} seq=0x{seq:02x} content={} crc={} computed_crc={} crc_ok={} term=0x{term:02x}",
        len,
        hex_bytes(content),
        hex_bytes(crc),
        hex_bytes(&computed_crc),
        crc == computed_crc
    );

    if content.first().copied() == Some(0x00) {
        print_identify_response(content);
        true
    } else {
        false
    }
}

fn print_identify_response(content: &[u8]) {
    let mut offset = 1;
    let Some(response_offset) = decode_vlq(content, &mut offset) else {
        println!("identify_response decode failed: missing offset");
        return;
    };

    let Some(&data_len) = content.get(offset) else {
        println!("identify_response decode failed: missing data length");
        return;
    };
    offset += 1;

    let data_len = data_len as usize;
    let Some(data) = content.get(offset..offset + data_len) else {
        println!("identify_response decode failed: truncated data");
        return;
    };

    println!(
        "identify_response offset={} data_len={} data={}",
        response_offset,
        data_len,
        hex_bytes(data)
    );

    if data.starts_with(&[0x78, 0xda]) || data.starts_with(&[0x78, 0x9c]) {
        println!("identify_response data looks like a zlib stream start");
    }
}

fn crc16_ccitt_klipper(bytes: &[u8]) -> u16 {
    let mut crc = 0xffffu16;

    for byte in bytes {
        crc ^= *byte as u16;
        for _ in 0..8 {
            if crc & 1 != 0 {
                crc = (crc >> 1) ^ 0x8408;
            } else {
                crc >>= 1;
            }
        }
    }

    crc
}

fn hex_bytes(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_initial_identify_request() {
        let frame = encode_identify_request(0x10, 0, 40).unwrap();

        assert_eq!(&frame[..5], [0x08, 0x10, 0x01, 0x00, 0x28]);
        assert_eq!(frame.last().copied(), Some(0x7e));
        assert_eq!(
            crc16_ccitt_klipper(&frame[..5]).to_be_bytes(),
            [frame[5], frame[6]]
        );
    }

    #[test]
    fn validates_known_ack_crc() {
        assert_eq!(crc16_ccitt_klipper(&[0x05, 0x10]), 0x9e81);
    }
}
