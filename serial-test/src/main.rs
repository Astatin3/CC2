mod protocol;
mod strace_parser;

use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};

use protocol::{McuCommand, McuFrame, hex_bytes, split_frames};
use strace_parser::{TracePacket, parse_trace_line};

const DEFAULT_PROCESS: &str = "elegoo_printer";
const DEFAULT_FD: u32 = 26;

fn main() -> anyhow::Result<()> {
    let config = Config::from_args()?;

    eprintln!(
        "starting passive trace: process={} fd={} strace -f -tt -xx -s 512 -e trace=read,write,ioctl",
        config.process, config.fd
    );

    let pid = pidof(&config.process)?;
    let mut strace = Command::new("strace")
        .args([
            "-f",
            "-tt",
            "-xx",
            "-s",
            "512",
            "-e",
            "trace=read,write,ioctl",
            "-p",
            &pid,
        ])
        .stderr(Stdio::piped())
        .spawn()?;

    let stderr = strace
        .stderr
        .take()
        .ok_or_else(|| anyhow::anyhow!("failed to capture strace stderr"))?;

    for line in BufReader::new(stderr).lines() {
        let line = line?;
        if let Some(packet) = parse_trace_line(&line, config.fd) {
            log_packet(&packet);
        }
    }

    let status = strace.wait()?;
    if !status.success() {
        anyhow::bail!("strace exited with {status}");
    }

    Ok(())
}

#[derive(Debug)]
struct Config {
    process: String,
    fd: u32,
}

impl Config {
    fn from_args() -> anyhow::Result<Self> {
        let mut process = DEFAULT_PROCESS.to_string();
        let mut fd = DEFAULT_FD;
        let mut args = std::env::args().skip(1);

        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--process" | "-p" => {
                    process = args
                        .next()
                        .ok_or_else(|| anyhow::anyhow!("missing value for {arg}"))?;
                }
                "--fd" | "-f" => {
                    let value = args
                        .next()
                        .ok_or_else(|| anyhow::anyhow!("missing value for {arg}"))?;
                    fd = value.parse()?;
                }
                "--help" | "-h" => {
                    print_help();
                    std::process::exit(0);
                }
                _ => anyhow::bail!("unknown argument: {arg}"),
            }
        }

        Ok(Self { process, fd })
    }
}

fn print_help() {
    println!(
        "Usage: serial-test [--process elegoo_printer] [--fd 26]\n\n\
         Passively wraps:\n\
         strace -f -tt -xx -s 512 -e trace=read,write,ioctl -p $(pidof PROCESS)\n\n\
         It filters the selected fd in-process and prints named MCU protocol frames."
    );
}

fn pidof(process: &str) -> anyhow::Result<String> {
    let output = Command::new("pidof").arg(process).output()?;
    if !output.status.success() {
        anyhow::bail!("pidof {process} failed; is the process running?");
    }

    let pid = String::from_utf8(output.stdout)?
        .split_whitespace()
        .next()
        .ok_or_else(|| anyhow::anyhow!("pidof {process} returned no pid"))?
        .to_string();

    Ok(pid)
}

fn log_packet(packet: &TracePacket) {
    for frame in split_frames(&packet.bytes) {
        let frame = McuFrame::parse(packet.direction, frame);
        let timestamp = packet.timestamp.as_deref().unwrap_or("-");
        let result = packet
            .result_len
            .map(|len| format!(" syscall_len={len}"))
            .unwrap_or_default();

        println!(
            "{timestamp} {} {} len={} seq=0x{:02x} message_id={} {} crc={} computed_crc={} crc_ok={} term=0x{:02x} messages={}{}",
            packet.direction,
            frame.command,
            frame.len,
            frame.seq,
            message_id(&frame),
            command_data(&frame),
            hex_bytes(&frame.trailer),
            hex_bytes(&frame.computed_crc),
            frame.crc_valid,
            frame.terminator,
            format_messages(&frame),
            result
        );
    }
}

fn message_id(frame: &McuFrame) -> String {
    frame
        .message_id
        .map(|message_id| format!("0x{message_id:02x}"))
        .unwrap_or_else(|| "-".to_string())
}

fn format_messages(frame: &McuFrame) -> String {
    if frame.messages.is_empty() {
        return "[]".to_string();
    }

    let messages = frame
        .messages
        .iter()
        .map(|message| {
            format!(
                "{{id=0x{:02x}, payload={}}}",
                message.message_id,
                hex_bytes(&message.payload)
            )
        })
        .collect::<Vec<_>>()
        .join(", ");

    format!("[{messages}]")
}

fn command_data(frame: &McuFrame) -> String {
    match &frame.command {
        McuCommand::HostPeriodicQuery(data) => format!("data={data}"),
        McuCommand::HostMotionControl(data) => format!("data={data}"),
        McuCommand::HostMotionProgram(data) => format!("data={data}"),
        McuCommand::HostControlResponse(data) => format!("data={data}"),
        McuCommand::HostAction(data) => format!("data={data}"),
        McuCommand::DeviceQueryResponse(data) => format!("data={data}"),
        McuCommand::DeviceHomingAck(data) => format!("data={data}"),
        McuCommand::DeviceHomingStatus(data) => format!("data={data}"),
        McuCommand::DevicePeriodicStatus(data) => format!("data={data}"),
        McuCommand::DeviceActionResponse(data) => format!("data={data}"),
        McuCommand::DeviceSensorStatus(data) => format!("data={data}"),
        McuCommand::TransportAck(data) => format!("crc={}", hex_bytes(&data.crc)),
        McuCommand::Unknown(_) => format!(
            "content={} raw={}",
            hex_bytes(&frame.content),
            hex_bytes(&frame.raw)
        ),
        McuCommand::MalformedFrame => format!("raw={}", hex_bytes(&frame.raw)),
    }
}
