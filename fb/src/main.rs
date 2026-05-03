use std::error::Error;

use crate::framebuffer::FrameBuffer;

mod framebuffer;

fn main() -> Result<(), Box<dyn Error>> {
    let mut fb = FrameBuffer::new("/dev/fb0")?;

    println!("Display started at resolution ({},{})!", fb.xres, fb.yres);

    let buffer_size = fb.xres * fb.yres;

    let mut imgbuffer;

    let mut t: u8 = 0;

    loop {
        imgbuffer = (0..buffer_size)
            .map(|n| {
                let x = n % fb.xres;
                let y = n / fb.xres;
                let r = scale_to_u8(x, fb.xres).wrapping_add(t);
                let g = scale_to_u8(y, fb.yres).wrapping_add(t);

                (r, g, 0u8)
            })
            .flat_map(|(r, g, b)| [r, g, b])
            .collect::<Vec<u8>>();

        fb.display_frame(&imgbuffer, fb.xres, fb.yres)?;

        t = t.wrapping_add(1);
    }

    // Ok(())
}

fn scale_to_u8(value: u32, size: u32) -> u8 {
    ((value as u64 * 255) / (size - 1) as u64) as u8
}
