use memmap2::{MmapMut, MmapOptions};
use std::fs::{File, OpenOptions};
use std::io::{self, Error, ErrorKind};
use std::os::unix::io::AsRawFd;

const FBIOGET_VSCREENINFO: libc::c_ulong = 0x4600;
const FBIOGET_FSCREENINFO: libc::c_ulong = 0x4602;

#[repr(C)]
#[derive(Debug, Copy, Clone)]
struct FbFixScreenInfo {
    id: [u8; 16],
    smem_start: libc::c_ulong,
    smem_len: u32,
    fb_type: u32,
    type_aux: u32,
    visual: u32,
    xpanstep: u16,
    ypanstep: u16,
    ywrapstep: u16,
    line_length: u32,
    mmio_start: libc::c_ulong,
    mmio_len: u32,
    accel: u32,
    capabilities: u16,
    reserved: [u16; 2],
}

#[repr(C)]
#[derive(Debug, Copy, Clone)]
struct FbVarScreenInfo {
    xres: u32,
    yres: u32,
    xres_virtual: u32,
    yres_virtual: u32,
    xoffset: u32,
    yoffset: u32,
    bits_per_pixel: u32,
    grayscale: u32,
    red: FbBitfield,
    green: FbBitfield,
    blue: FbBitfield,
    transp: FbBitfield,
    nonstd: u32,
    activate: u32,
    height: u32,
    width: u32,
    accel_flags: u32,
    pixclock: u32,
    left_margin: u32,
    right_margin: u32,
    upper_margin: u32,
    lower_margin: u32,
    hsync_len: u32,
    vsync_len: u32,
    sync: u32,
    vmode: u32,
    rotate: u32,
    colorspace: u32,
    reserved: [u32; 4],
}

#[repr(C)]
#[derive(Debug, Copy, Clone)]
struct FbBitfield {
    offset: u32,
    length: u32,
    msb_right: u32,
}

#[derive(Debug, Copy, Clone)]
enum PixelFormat {
    Rgb565,
    Native32,
    Native24,
    Native16,
}

pub struct FrameBuffer {
    _file: File,
    fb_map: MmapMut,
    pub xres: u32,
    pub yres: u32,
    xoffset: u32,
    yoffset: u32,
    line_length: usize,
    bytes_per_pixel: usize,
    red: FbBitfield,
    green: FbBitfield,
    blue: FbBitfield,
    transp: FbBitfield,
    format: PixelFormat,
}

impl FrameBuffer {
    pub fn new(device: &str) -> io::Result<Self> {
        let file = OpenOptions::new().read(true).write(true).open(device)?;

        let mut fix_info = unsafe { std::mem::zeroed::<FbFixScreenInfo>() };
        ioctl_read(file.as_raw_fd(), FBIOGET_FSCREENINFO, &mut fix_info)?;

        let mut var_info = unsafe { std::mem::zeroed::<FbVarScreenInfo>() };
        ioctl_read(file.as_raw_fd(), FBIOGET_VSCREENINFO, &mut var_info)?;

        let bytes_per_pixel = match var_info.bits_per_pixel {
            16 => 2,
            24 => 3,
            32 => 4,
            bpp => {
                return Err(Error::new(
                    ErrorKind::Unsupported,
                    format!("unsupported framebuffer depth: {bpp} bpp"),
                ));
            }
        };

        if fix_info.smem_len == 0 {
            return Err(Error::new(
                ErrorKind::InvalidData,
                "framebuffer reports zero memory length",
            ));
        }

        let line_length = fix_info.line_length as usize;
        if line_length == 0 {
            return Err(Error::new(
                ErrorKind::InvalidData,
                "framebuffer reports zero line length",
            ));
        }

        let format = detect_format(
            var_info.bits_per_pixel,
            var_info.red,
            var_info.green,
            var_info.blue,
        );
        let fb_map = unsafe {
            MmapOptions::new()
                .len(fix_info.smem_len as usize)
                .map_mut(&file)?
        };

        Ok(FrameBuffer {
            _file: file,
            fb_map,
            xres: var_info.xres,
            yres: var_info.yres,
            xoffset: var_info.xoffset,
            yoffset: var_info.yoffset,
            line_length,
            bytes_per_pixel,
            red: var_info.red,
            green: var_info.green,
            blue: var_info.blue,
            transp: var_info.transp,
            format,
        })
    }

    pub fn display_frame(&mut self, frame: &[u8], width: u32, height: u32) -> io::Result<()> {
        self.display_frame_at(frame, width, height, 0, 0)
    }

    pub fn display_frame_at(
        &mut self,
        frame: &[u8],
        width: u32,
        height: u32,
        offset_x: i32,
        offset_y: i32,
    ) -> io::Result<()> {
        let expected_len = rgb888_len(width, height)?;
        if frame.len() < expected_len {
            return Err(Error::new(
                ErrorKind::InvalidInput,
                format!(
                    "frame is too small for {width}x{height} RGB888: {} < {expected_len}",
                    frame.len()
                ),
            ));
        }

        let Some(clip) = Clip::new(width, height, self.xres, self.yres, offset_x, offset_y) else {
            return Ok(());
        };

        let xoffset = self.xoffset as usize;
        let yoffset = self.yoffset as usize;
        let line_length = self.line_length;
        let bpp = self.bytes_per_pixel;
        let red = self.red;
        let green = self.green;
        let blue = self.blue;
        let transp = self.transp;
        let format = self.format;

        for row in 0..clip.height {
            let src_y = clip.src_y + row;
            let dst_y = clip.dst_y + row;
            let src_start = ((src_y * width + clip.src_x) * 3) as usize;
            let src_end = src_start + (clip.width * 3) as usize;
            let src = &frame[src_start..src_end];

            let dst_start =
                (dst_y as usize + yoffset) * line_length + (clip.dst_x as usize + xoffset) * bpp;
            let dst_end = dst_start + clip.width as usize * bpp;
            if dst_end > self.fb_map.len() {
                return Err(Error::new(
                    ErrorKind::InvalidData,
                    "visible framebuffer area exceeds mapped memory",
                ));
            }

            write_rgb888_row(
                src,
                &mut self.fb_map[dst_start..dst_end],
                format,
                red,
                green,
                blue,
                transp,
            );
        }

        // fbdev mappings are device memory. The writes above are the update;
        // msync can return EINVAL on framebuffer mappings even after pixels land.
        Ok(())
    }
}

fn ioctl_read<T>(fd: std::os::fd::RawFd, request: libc::c_ulong, value: &mut T) -> io::Result<()> {
    let result = unsafe { libc::ioctl(fd, request.try_into().unwrap(), value as *mut T) };
    if result < 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}

fn detect_format(
    bits_per_pixel: u32,
    red: FbBitfield,
    green: FbBitfield,
    blue: FbBitfield,
) -> PixelFormat {
    match bits_per_pixel {
        16 if red.offset == 11
            && red.length == 5
            && green.offset == 5
            && green.length == 6
            && blue.offset == 0
            && blue.length == 5 =>
        {
            PixelFormat::Rgb565
        }
        16 => PixelFormat::Native16,
        24 => PixelFormat::Native24,
        32 => PixelFormat::Native32,
        _ => unreachable!("unsupported framebuffer depth was rejected earlier"),
    }
}

fn rgb888_len(width: u32, height: u32) -> io::Result<usize> {
    width
        .checked_mul(height)
        .and_then(|pixels| pixels.checked_mul(3))
        .map(|len| len as usize)
        .ok_or_else(|| Error::new(ErrorKind::InvalidInput, "RGB888 frame dimensions overflow"))
}

fn write_rgb888_row(
    src: &[u8],
    dst: &mut [u8],
    format: PixelFormat,
    red: FbBitfield,
    green: FbBitfield,
    blue: FbBitfield,
    transp: FbBitfield,
) {
    match format {
        PixelFormat::Rgb565 => write_rgb565_row(src, dst),
        PixelFormat::Native16 => write_native_row::<2>(src, dst, red, green, blue, transp),
        PixelFormat::Native24 => write_native_row::<3>(src, dst, red, green, blue, transp),
        PixelFormat::Native32 => write_native_row::<4>(src, dst, red, green, blue, transp),
    }
}

fn write_rgb565_row(src: &[u8], dst: &mut [u8]) {
    for (rgb, out) in src.chunks_exact(3).zip(dst.chunks_exact_mut(2)) {
        let r = (rgb[0] as u16 >> 3) << 11;
        let g = (rgb[1] as u16 >> 2) << 5;
        let b = rgb[2] as u16 >> 3;
        out.copy_from_slice(&(r | g | b).to_ne_bytes());
    }
}

fn write_native_row<const N: usize>(
    src: &[u8],
    dst: &mut [u8],
    red: FbBitfield,
    green: FbBitfield,
    blue: FbBitfield,
    transp: FbBitfield,
) {
    for (rgb, out) in src.chunks_exact(3).zip(dst.chunks_exact_mut(N)) {
        let pixel = pack_pixel(rgb[0], rgb[1], rgb[2], red, green, blue, transp);
        out.copy_from_slice(&pixel.to_ne_bytes()[..N]);
    }
}

fn pack_pixel(
    r: u8,
    g: u8,
    b: u8,
    red: FbBitfield,
    green: FbBitfield,
    blue: FbBitfield,
    transp: FbBitfield,
) -> u32 {
    pack_channel(r, red) | pack_channel(g, green) | pack_channel(b, blue) | pack_alpha(transp)
}

fn pack_channel(value: u8, field: FbBitfield) -> u32 {
    if field.length == 0 {
        return 0;
    }

    let max = (1u32 << field.length) - 1;
    let scaled = (value as u32 * max + 127) / 255;
    scaled << field.offset
}

fn pack_alpha(field: FbBitfield) -> u32 {
    if field.length == 0 {
        return 0;
    }

    ((1u32 << field.length) - 1) << field.offset
}

struct Clip {
    src_x: u32,
    src_y: u32,
    dst_x: u32,
    dst_y: u32,
    width: u32,
    height: u32,
}

impl Clip {
    fn new(
        src_width: u32,
        src_height: u32,
        dst_width: u32,
        dst_height: u32,
        offset_x: i32,
        offset_y: i32,
    ) -> Option<Self> {
        if src_width == 0 || src_height == 0 || dst_width == 0 || dst_height == 0 {
            return None;
        }

        let src_x = offset_x.saturating_neg().max(0) as u32;
        let src_y = offset_y.saturating_neg().max(0) as u32;
        let dst_x = offset_x.max(0) as u32;
        let dst_y = offset_y.max(0) as u32;

        if src_x >= src_width || src_y >= src_height || dst_x >= dst_width || dst_y >= dst_height {
            return None;
        }

        let width = (src_width - src_x).min(dst_width - dst_x);
        let height = (src_height - src_y).min(dst_height - dst_y);
        if width == 0 || height == 0 {
            return None;
        }

        Some(Clip {
            src_x,
            src_y,
            dst_x,
            dst_y,
            width,
            height,
        })
    }
}
