use std::sync::atomic::{AtomicBool, Ordering};

pub static INTERRUPTED: AtomicBool = AtomicBool::new(false);

extern "C" fn handle_sigint(_sig: i32) {
    INTERRUPTED.store(true, Ordering::SeqCst);
}

#[repr(C)]
#[derive(Clone, Copy)]
struct Termios {
    c_iflag: u32,
    c_oflag: u32,
    c_cflag: u32,
    c_lflag: u32,
    c_line: u8,
    c_cc: [u8; 32],
    c_ispeed: u32,
    c_ospeed: u32,
}

const ICANON: u32 = 0o0000002;
const ECHO: u32 = 0o0000010;
const TCSANOW: i32 = 0;
const SIGINT: i32 = 2;

extern "C" {
    fn isatty(fd: i32) -> i32;
    fn tcgetattr(fd: i32, termios_p: *mut Termios) -> i32;
    fn tcsetattr(fd: i32, optional_actions: i32, termios_p: *const Termios) -> i32;
    fn signal(signum: i32, handler: extern "C" fn(i32)) -> usize;
}

pub struct RawTermGuard {
    fd: i32,
    orig: Option<Termios>,
}

impl RawTermGuard {
    pub fn new() -> Self {
        INTERRUPTED.store(false, Ordering::SeqCst);
        unsafe {
            signal(SIGINT, handle_sigint);
            let fd = 0;
            if isatty(fd) == 1 {
                let mut orig: Termios = std::mem::zeroed();
                if tcgetattr(fd, &mut orig) == 0 {
                    let mut raw = orig;
                    raw.c_lflag &= !(ICANON | ECHO);
                    tcsetattr(fd, TCSANOW, &raw);
                    return Self {
                        fd,
                        orig: Some(orig),
                    };
                }
            }
            Self { fd, orig: None }
        }
    }

    pub fn is_tty(&self) -> bool {
        self.orig.is_some()
    }
}

impl Drop for RawTermGuard {
    fn drop(&mut self) {
        if let Some(orig) = self.orig {
            unsafe {
                tcsetattr(self.fd, TCSANOW, &orig);
            }
        }
    }
}
