use crate::protocol::Direction;

#[derive(Debug)]
pub struct TracePacket {
    pub timestamp: Option<String>,
    pub direction: Direction,
    pub bytes: Vec<u8>,
    pub result_len: Option<usize>,
}

pub fn parse_trace_line(line: &str, fd: u32) -> Option<TracePacket> {
    let syscall_start = line
        .find("read(")
        .or_else(|| line.find("write("))
        .or_else(|| line.find("ioctl("))?;
    let syscall = &line[syscall_start..];
    let timestamp = line[..syscall_start]
        .split_whitespace()
        .last()
        .map(str::to_string);

    let (direction, rest) = if let Some(rest) = syscall.strip_prefix("read(") {
        (Direction::DeviceRead, rest)
    } else if let Some(rest) = syscall.strip_prefix("write(") {
        (Direction::HostWrite, rest)
    } else {
        let rest = syscall.strip_prefix("ioctl(")?;
        (Direction::Ioctl, rest)
    };

    let fd_prefix = format!("{fd},");
    if !rest.starts_with(&fd_prefix) {
        return None;
    }

    if direction == Direction::Ioctl {
        println!(
            "{} ioctl fd={fd}: {line}",
            timestamp.as_deref().unwrap_or("-")
        );
        return None;
    }

    let bytes = parse_bytes_argument(rest)?;
    let result_len = parse_result_len(syscall);

    Some(TracePacket {
        timestamp,
        direction,
        bytes,
        result_len,
    })
}

fn parse_bytes_argument(rest: &str) -> Option<Vec<u8>> {
    let first_quote = rest.find('"')?;
    let encoded = &rest[first_quote + 1..];
    let end_quote = find_unescaped_quote(encoded)?;
    Some(decode_strace_bytes(&encoded[..end_quote]))
}

fn find_unescaped_quote(s: &str) -> Option<usize> {
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'"' => return Some(i),
            b'\\' => i += 2,
            _ => i += 1,
        }
    }
    None
}

fn decode_strace_bytes(s: &str) -> Vec<u8> {
    let bytes = s.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;

    while i < bytes.len() {
        if bytes[i] == b'\\' && i + 1 < bytes.len() {
            match bytes[i + 1] {
                b'x' if i + 3 < bytes.len() => {
                    if let Ok(value) = u8::from_str_radix(&s[i + 2..i + 4], 16) {
                        out.push(value);
                        i += 4;
                        continue;
                    }
                }
                b'n' => {
                    out.push(b'\n');
                    i += 2;
                    continue;
                }
                b'r' => {
                    out.push(b'\r');
                    i += 2;
                    continue;
                }
                b't' => {
                    out.push(b'\t');
                    i += 2;
                    continue;
                }
                b'\\' => {
                    out.push(b'\\');
                    i += 2;
                    continue;
                }
                b'"' => {
                    out.push(b'"');
                    i += 2;
                    continue;
                }
                b'0'..=b'7' => {
                    let mut end = i + 2;
                    while end < bytes.len() && end < i + 4 && bytes[end].is_ascii_digit() {
                        end += 1;
                    }
                    if let Ok(value) = u8::from_str_radix(&s[i + 1..end], 8) {
                        out.push(value);
                        i = end;
                        continue;
                    }
                }
                value => {
                    out.push(value);
                    i += 2;
                    continue;
                }
            }
        }

        out.push(bytes[i]);
        i += 1;
    }

    out
}

fn parse_result_len(syscall: &str) -> Option<usize> {
    let result = syscall.rsplit_once(" = ")?.1;
    result.split_whitespace().next()?.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_write_line() {
        let line = r#"12:34:56.789012 write(26, "\x06\x14\x05\x4a\xb6\x7e", 6) = 6"#;
        let packet = parse_trace_line(line, 26).unwrap();

        assert_eq!(packet.direction, Direction::HostWrite);
        assert_eq!(packet.bytes, [0x06, 0x14, 0x05, 0x4a, 0xb6, 0x7e]);
        assert_eq!(packet.result_len, Some(6));
    }
}
