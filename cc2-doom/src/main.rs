mod doom;
#[cfg(all(target_os = "linux", target_arch = "arm"))]
mod framebuffer;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    doom::run()
}
