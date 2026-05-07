use std::error::Error;
use std::ffi::CString;
use std::fs;
use std::os::raw::{c_char, c_int, c_uchar};
use std::path::PathBuf;

const DOOM_WIDTH: u32 = 320;
const DOOM_HEIGHT: u32 = 200;
const DOOM_FLAGS_HIDE_MOUSE_OPTIONS: c_int = 1;
const DOOM_FLAGS_HIDE_SOUND_OPTIONS: c_int = 2;
const DOOM_FLAGS_HIDE_MUSIC_OPTIONS: c_int = 4;
const EMBEDDED_DOOM1_WAD: &[u8] = include_bytes!("../doom1.wad");

unsafe extern "C" {
    fn doom_set_resolution(width: c_int, height: c_int);
    fn doom_init(argc: c_int, argv: *mut *mut c_char, flags: c_int);
    fn doom_update();
    fn doom_get_framebuffer(channels: c_int) -> *const c_uchar;
    #[cfg(all(target_os = "linux", target_arch = "arm"))]
    fn doom_key_down(key: c_int);
    #[cfg(all(target_os = "linux", target_arch = "arm"))]
    fn doom_key_up(key: c_int);
}

pub fn run() -> Result<(), Box<dyn Error>> {
    let mut doom = Doom::new()?;
    video::run(&mut doom)
}

pub struct Doom {
    _args: Vec<CString>,
    _argv: Vec<*mut c_char>,
}

impl Doom {
    fn new() -> Result<Self, Box<dyn Error>> {
        let args = args_with_embedded_iwad()?;
        let mut args = args
            .into_iter()
            .map(|arg| CString::new(arg).map_err(|err| -> Box<dyn Error> { Box::new(err) }))
            .collect::<Result<Vec<_>, _>>()?;
        let mut argv = args
            .iter_mut()
            .map(|arg| arg.as_ptr() as *mut c_char)
            .collect::<Vec<_>>();

        unsafe {
            doom_set_resolution(DOOM_WIDTH as c_int, DOOM_HEIGHT as c_int);
            doom_init(
                argv.len() as c_int,
                argv.as_mut_ptr(),
                DOOM_FLAGS_HIDE_MOUSE_OPTIONS
                    | DOOM_FLAGS_HIDE_SOUND_OPTIONS
                    | DOOM_FLAGS_HIDE_MUSIC_OPTIONS,
            );
        }

        Ok(Self {
            _args: args,
            _argv: argv,
        })
    }

    fn update(&mut self) {
        unsafe {
            doom_update();
        }
    }

    #[cfg(all(target_os = "linux", target_arch = "arm"))]
    fn key_down(&mut self, key: c_int) {
        unsafe {
            doom_key_down(key);
        }
    }

    #[cfg(all(target_os = "linux", target_arch = "arm"))]
    fn key_up(&mut self, key: c_int) {
        unsafe {
            doom_key_up(key);
        }
    }

    fn framebuffer_rgb(&self) -> &'static [u8] {
        let ptr = unsafe { doom_get_framebuffer(3) };
        assert!(!ptr.is_null(), "Doom returned a null framebuffer");
        unsafe { std::slice::from_raw_parts(ptr, (DOOM_WIDTH * DOOM_HEIGHT * 3) as usize) }
    }
}

fn args_with_embedded_iwad() -> Result<Vec<String>, Box<dyn Error>> {
    let mut args = std::env::args().collect::<Vec<_>>();
    if has_iwad_arg(&args) {
        return Ok(args);
    }

    let iwad = embedded_iwad_path();
    if fs::read(&iwad).ok().as_deref() != Some(EMBEDDED_DOOM1_WAD) {
        fs::write(&iwad, EMBEDDED_DOOM1_WAD)?;
    }

    args.push("-iwad".to_string());
    args.push(iwad.to_string_lossy().into_owned());
    Ok(args)
}

fn has_iwad_arg(args: &[String]) -> bool {
    args.iter().any(|arg| arg.eq_ignore_ascii_case("-iwad"))
}

fn embedded_iwad_path() -> PathBuf {
    std::env::temp_dir().join("cc2-doom-doom1.wad")
}

#[cfg(all(target_os = "linux", target_arch = "arm"))]
mod video {
    use std::error::Error;
    use std::io;
    use std::mem;
    use std::os::fd::RawFd;

    use crate::doom::{DOOM_HEIGHT, DOOM_WIDTH, Doom};
    use crate::framebuffer::FrameBuffer;

    const DOOM_KEY_TAB: i32 = 9;
    const DOOM_KEY_ENTER: i32 = 13;
    const DOOM_KEY_ESCAPE: i32 = 27;
    const DOOM_KEY_SPACE: i32 = 32;
    const DOOM_KEY_APOSTROPHE: i32 = b'\'' as i32;
    const DOOM_KEY_COMMA: i32 = b',' as i32;
    const DOOM_KEY_MINUS: i32 = b'-' as i32;
    const DOOM_KEY_PERIOD: i32 = b'.' as i32;
    const DOOM_KEY_SLASH: i32 = b'/' as i32;
    const DOOM_KEY_SEMICOLON: i32 = b';' as i32;
    const DOOM_KEY_EQUALS: i32 = b'=' as i32;
    const DOOM_KEY_LEFT_BRACKET: i32 = b'[' as i32;
    const DOOM_KEY_RIGHT_BRACKET: i32 = b']' as i32;
    const DOOM_KEY_BACKSPACE: i32 = 127;
    const DOOM_KEY_CTRL: i32 = 0x80 + 0x1d;
    const DOOM_KEY_LEFT_ARROW: i32 = 0xac;
    const DOOM_KEY_UP_ARROW: i32 = 0xad;
    const DOOM_KEY_RIGHT_ARROW: i32 = 0xae;
    const DOOM_KEY_DOWN_ARROW: i32 = 0xaf;
    const DOOM_KEY_SHIFT: i32 = 0x80 + 0x36;
    const DOOM_KEY_ALT: i32 = 0x80 + 0x38;
    const DOOM_KEY_F1: i32 = 0x80 + 0x3b;
    const DOOM_KEY_F2: i32 = 0x80 + 0x3c;
    const DOOM_KEY_F3: i32 = 0x80 + 0x3d;
    const DOOM_KEY_F4: i32 = 0x80 + 0x3e;
    const DOOM_KEY_F5: i32 = 0x80 + 0x3f;
    const DOOM_KEY_F6: i32 = 0x80 + 0x40;
    const DOOM_KEY_F7: i32 = 0x80 + 0x41;
    const DOOM_KEY_F8: i32 = 0x80 + 0x42;
    const DOOM_KEY_F9: i32 = 0x80 + 0x43;
    const DOOM_KEY_F10: i32 = 0x80 + 0x44;
    const DOOM_KEY_F11: i32 = 0x80 + 0x57;
    const DOOM_KEY_F12: i32 = 0x80 + 0x58;
    const DOOM_KEY_PAUSE: i32 = 0xff;

    pub fn run(doom: &mut Doom) -> Result<(), Box<dyn Error>> {
        let mut fb = FrameBuffer::new("/dev/fb0")?;
        let mut keyboard = StdinKeyboard::new()?;
        let offset_x = ((fb.xres as i32) - (DOOM_WIDTH as i32)) / 2;
        let offset_y = ((fb.yres as i32) - (DOOM_HEIGHT as i32)) / 2;

        loop {
            keyboard.poll(doom)?;
            doom.update();
            fb.display_frame_at(
                doom.framebuffer_rgb(),
                DOOM_WIDTH,
                DOOM_HEIGHT,
                offset_x,
                offset_y,
            )?;
        }
    }

    struct StdinKeyboard {
        fd: RawFd,
        original_termios: Option<libc::termios>,
        pressed: Vec<i32>,
    }

    impl StdinKeyboard {
        fn new() -> io::Result<Self> {
            let fd = libc::STDIN_FILENO;
            let original_termios = set_raw_nonblocking(fd)?;
            Ok(Self {
                fd,
                original_termios,
                pressed: Vec::new(),
            })
        }

        fn poll(&mut self, doom: &mut Doom) -> io::Result<()> {
            let mut bytes = [0u8; 64];
            let len = read_available(self.fd, &mut bytes)?;
            if len == 0 {
                self.release_all(doom);
                return Ok(());
            }

            self.release_all(doom);

            let mut i = 0;
            while i < len {
                let (key, consumed) = parse_key(&bytes[i..len]);
                i += consumed.max(1);

                if let Some(key) = key {
                    doom.key_down(key);
                    self.pressed.push(key);
                }
            }

            Ok(())
        }

        fn release_all(&mut self, doom: &mut Doom) {
            for key in self.pressed.drain(..) {
                doom.key_up(key);
            }
        }
    }

    impl Drop for StdinKeyboard {
        fn drop(&mut self) {
            if let Some(termios) = self.original_termios {
                unsafe {
                    libc::tcsetattr(self.fd, libc::TCSANOW, &termios);
                }
            }
        }
    }

    fn set_raw_nonblocking(fd: RawFd) -> io::Result<Option<libc::termios>> {
        let mut termios = unsafe { mem::zeroed::<libc::termios>() };
        let has_tty = unsafe { libc::tcgetattr(fd, &mut termios) } == 0;
        let original = has_tty.then_some(termios);

        if has_tty {
            let mut raw = termios;
            raw.c_lflag &= !(libc::ICANON | libc::ECHO | libc::ISIG);
            raw.c_iflag &= !(libc::IXON | libc::ICRNL);
            raw.c_cc[libc::VMIN] = 0;
            raw.c_cc[libc::VTIME] = 0;

            if unsafe { libc::tcsetattr(fd, libc::TCSANOW, &raw) } < 0 {
                return Err(io::Error::last_os_error());
            }
        }

        let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
        if flags < 0 {
            return Err(io::Error::last_os_error());
        }

        if unsafe { libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) } < 0 {
            return Err(io::Error::last_os_error());
        }

        Ok(original)
    }

    fn read_available(fd: RawFd, out: &mut [u8]) -> io::Result<usize> {
        let result = unsafe { libc::read(fd, out.as_mut_ptr() as *mut libc::c_void, out.len()) };
        if result < 0 {
            let err = io::Error::last_os_error();
            return match err.kind() {
                io::ErrorKind::WouldBlock => Ok(0),
                _ => Err(err),
            };
        }

        Ok(result as usize)
    }

    fn parse_key(bytes: &[u8]) -> (Option<i32>, usize) {
        if bytes.is_empty() {
            return (None, 0);
        }

        if bytes[0] == b'\x1b' {
            return parse_escape(bytes);
        }

        (byte_to_doom_key(bytes[0]), 1)
    }

    fn parse_escape(bytes: &[u8]) -> (Option<i32>, usize) {
        if bytes.len() >= 3 && bytes[1] == b'[' {
            return match bytes[2] {
                b'A' => (Some(DOOM_KEY_UP_ARROW), 3),
                b'B' => (Some(DOOM_KEY_DOWN_ARROW), 3),
                b'C' => (Some(DOOM_KEY_RIGHT_ARROW), 3),
                b'D' => (Some(DOOM_KEY_LEFT_ARROW), 3),
                b'Z' => (Some(DOOM_KEY_TAB), 3),
                b'1' | b'2' | b'3' | b'4' | b'5' | b'6' if bytes.len() >= 4 => {
                    parse_csi_tilde(bytes)
                }
                _ => (Some(DOOM_KEY_ESCAPE), 1),
            };
        }

        if bytes.len() >= 3 && bytes[1] == b'O' {
            return match bytes[2] {
                b'P' => (Some(DOOM_KEY_F1), 3),
                b'Q' => (Some(DOOM_KEY_F2), 3),
                b'R' => (Some(DOOM_KEY_F3), 3),
                b'S' => (Some(DOOM_KEY_F4), 3),
                _ => (Some(DOOM_KEY_ESCAPE), 1),
            };
        }

        (Some(DOOM_KEY_ESCAPE), 1)
    }

    fn parse_csi_tilde(bytes: &[u8]) -> (Option<i32>, usize) {
        let tilde = bytes.iter().take(6).position(|&byte| byte == b'~');
        let Some(tilde) = tilde else {
            return (Some(DOOM_KEY_ESCAPE), 1);
        };

        let key = match &bytes[2..tilde] {
            b"15" => Some(DOOM_KEY_F5),
            b"17" => Some(DOOM_KEY_F6),
            b"18" => Some(DOOM_KEY_F7),
            b"19" => Some(DOOM_KEY_F8),
            b"20" => Some(DOOM_KEY_F9),
            b"21" => Some(DOOM_KEY_F10),
            b"23" => Some(DOOM_KEY_F11),
            b"24" => Some(DOOM_KEY_F12),
            _ => None,
        };
        (key, tilde + 1)
    }

    fn byte_to_doom_key(byte: u8) -> Option<i32> {
        match byte {
            b'\t' => Some(DOOM_KEY_TAB),
            b'\r' | b'\n' => Some(DOOM_KEY_ENTER),
            0x7f | 0x08 => Some(DOOM_KEY_BACKSPACE),
            0x01..=0x1a => Some((byte - 0x01 + b'a') as i32),
            b' ' => Some(DOOM_KEY_SPACE),
            b'\'' => Some(DOOM_KEY_APOSTROPHE),
            b',' => Some(DOOM_KEY_COMMA),
            b'-' => Some(DOOM_KEY_MINUS),
            b'.' => Some(DOOM_KEY_PERIOD),
            b'/' => Some(DOOM_KEY_SLASH),
            b'0'..=b'9' => Some(byte as i32),
            b';' => Some(DOOM_KEY_SEMICOLON),
            b'=' => Some(DOOM_KEY_EQUALS),
            b'[' => Some(DOOM_KEY_LEFT_BRACKET),
            b']' => Some(DOOM_KEY_RIGHT_BRACKET),
            b'a'..=b'z' => Some(byte as i32),
            b'A'..=b'Z' => Some(byte.to_ascii_lowercase() as i32),
            _ => None,
        }
    }
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
mod video {
    use std::error::Error;

    use pixels::{Pixels, SurfaceTexture};
    use winit::dpi::LogicalSize;
    use winit::event::{Event, WindowEvent};
    use winit::event_loop::{ControlFlow, EventLoop};
    use winit::window::WindowBuilder;

    use crate::doom::{DOOM_HEIGHT, DOOM_WIDTH, Doom};

    pub fn run(doom: &mut Doom) -> Result<(), Box<dyn Error>> {
        let event_loop = EventLoop::new()?;
        let window = WindowBuilder::new()
            .with_title("cc2-doom")
            .with_inner_size(LogicalSize::new(
                (DOOM_WIDTH * 3) as f64,
                (DOOM_HEIGHT * 3) as f64,
            ))
            .with_min_inner_size(LogicalSize::new(DOOM_WIDTH as f64, DOOM_HEIGHT as f64))
            .build(&event_loop)?;
        let window = Box::leak(Box::new(window));

        let surface = SurfaceTexture::new(DOOM_WIDTH * 3, DOOM_HEIGHT * 3, &*window);
        let mut pixels = Pixels::new(DOOM_WIDTH, DOOM_HEIGHT, surface)?;

        event_loop.run(move |event, elwt| {
            elwt.set_control_flow(ControlFlow::Poll);

            match event {
                Event::AboutToWait => {
                    doom.update();
                    copy_rgb_to_rgba(doom.framebuffer_rgb(), pixels.frame_mut());
                    if pixels.render().is_err() {
                        elwt.exit();
                    }
                }
                Event::WindowEvent {
                    event: WindowEvent::CloseRequested,
                    ..
                } => elwt.exit(),
                Event::WindowEvent {
                    event: WindowEvent::Resized(size),
                    ..
                } => {
                    let _ = pixels.resize_surface(size.width, size.height);
                }
                _ => {}
            }
        })?;

        Ok(())
    }

    fn copy_rgb_to_rgba(src: &[u8], dst: &mut [u8]) {
        for (rgb, rgba) in src.chunks_exact(3).zip(dst.chunks_exact_mut(4)) {
            rgba[0] = rgb[0];
            rgba[1] = rgb[1];
            rgba[2] = rgb[2];
            rgba[3] = 0xff;
        }
    }
}

#[cfg(not(any(
    all(target_os = "linux", target_arch = "arm"),
    all(target_os = "linux", target_arch = "x86_64")
)))]
mod video {
    use std::error::Error;
    use std::io::{Error as IoError, ErrorKind};

    use crate::doom::Doom;

    pub fn run(_doom: &mut Doom) -> Result<(), Box<dyn Error>> {
        Err(IoError::new(ErrorKind::Unsupported, "unsupported Doom video target").into())
    }
}
