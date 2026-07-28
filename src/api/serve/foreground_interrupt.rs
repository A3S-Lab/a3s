use std::io;

use tokio_util::sync::CancellationToken;

#[cfg(unix)]
use std::ffi::{CStr, OsStr};
#[cfg(unix)]
use std::fs::{File, OpenOptions};
#[cfg(unix)]
use std::os::fd::{AsRawFd, RawFd};
#[cfg(unix)]
use std::os::unix::ffi::OsStrExt;
#[cfg(unix)]
use std::os::unix::fs::OpenOptionsExt;
#[cfg(unix)]
use std::path::Path;
#[cfg(unix)]
use tokio::io::unix::AsyncFd;

/// Keeps terminal-generated interrupts available while foreground Web runs.
///
/// A TUI or terminal host can leave the controlling terminal in raw mode with
/// `ISIG` disabled. In that state Ctrl+C is delivered as the byte `0x03`
/// instead of `SIGINT`, so Tokio's signal listener never wakes. Foreground Web
/// promises Ctrl+C shutdown, therefore it both enables signal generation and
/// listens for the raw control byte if another process changes the mode later.
pub(super) struct InterruptSignalGuard {
    #[cfg(unix)]
    terminal: Option<UnixTerminalMode>,
    #[cfg(unix)]
    input_task: Option<tokio::task::JoinHandle<()>>,
}

#[cfg(unix)]
struct UnixTerminalMode {
    fd: RawFd,
    original: libc::termios,
}

impl InterruptSignalGuard {
    pub(super) fn enable(cancellation: CancellationToken) -> io::Result<Self> {
        #[cfg(unix)]
        {
            let mut guard = Self::enable_for_fd(libc::STDIN_FILENO)?;
            match start_interrupt_byte_monitor(libc::STDIN_FILENO, cancellation) {
                Ok(task) => guard.input_task = task,
                Err(error) => {
                    tracing::warn!(%error, "failed to monitor raw terminal Ctrl+C input");
                }
            }
            Ok(guard)
        }

        #[cfg(not(unix))]
        {
            let _ = cancellation;
            Ok(Self {})
        }
    }

    #[cfg(unix)]
    fn enable_for_fd(fd: RawFd) -> io::Result<Self> {
        if unsafe { libc::isatty(fd) } != 1 {
            return Ok(Self {
                terminal: None,
                input_task: None,
            });
        }

        let original = read_terminal_mode(fd)?;
        if original.c_lflag & libc::ISIG != 0 {
            return Ok(Self {
                terminal: None,
                input_task: None,
            });
        }

        let mut interruptible = original;
        interruptible.c_lflag |= libc::ISIG;
        write_terminal_mode(fd, &interruptible)?;
        Ok(Self {
            terminal: Some(UnixTerminalMode { fd, original }),
            input_task: None,
        })
    }
}

#[cfg(unix)]
impl Drop for InterruptSignalGuard {
    fn drop(&mut self) {
        if let Some(task) = self.input_task.take() {
            task.abort();
        }
        if let Some(terminal) = self.terminal.take() {
            if let Err(error) = write_terminal_mode(terminal.fd, &terminal.original) {
                tracing::warn!(%error, "failed to restore terminal interrupt mode");
            }
        }
    }
}

#[cfg(unix)]
fn start_interrupt_byte_monitor(
    fd: RawFd,
    cancellation: CancellationToken,
) -> io::Result<Option<tokio::task::JoinHandle<()>>> {
    if unsafe { libc::isatty(fd) } != 1
        || unsafe { libc::tcgetpgrp(fd) } != unsafe { libc::getpgrp() }
    {
        return Ok(None);
    }

    let input = AsyncFd::new(open_terminal_input(fd)?)?;
    Ok(Some(tokio::spawn(async move {
        monitor_interrupt_bytes(input, cancellation).await;
    })))
}

#[cfg(unix)]
fn open_terminal_input(fd: RawFd) -> io::Result<File> {
    let mut path = vec![0_u8; libc::PATH_MAX as usize];
    let result = unsafe { libc::ttyname_r(fd, path.as_mut_ptr().cast(), path.len()) };
    if result != 0 {
        return Err(io::Error::from_raw_os_error(result));
    }
    let path = unsafe { CStr::from_ptr(path.as_ptr().cast()) };
    let path = Path::new(OsStr::from_bytes(path.to_bytes()));
    OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NONBLOCK | libc::O_CLOEXEC)
        .open(path)
}

#[cfg(unix)]
async fn monitor_interrupt_bytes(input: AsyncFd<File>, cancellation: CancellationToken) {
    let mut buffer = [0_u8; 64];
    loop {
        let ready = tokio::select! {
            _ = cancellation.cancelled() => return,
            ready = input.readable() => ready,
        };
        let mut ready = match ready {
            Ok(ready) => ready,
            Err(error) => {
                tracing::debug!(%error, "raw terminal Ctrl+C monitor stopped");
                return;
            }
        };
        let read = ready.try_io(|source| {
            let read = unsafe {
                libc::read(
                    source.get_ref().as_raw_fd(),
                    buffer.as_mut_ptr().cast(),
                    buffer.len(),
                )
            };
            if read < 0 {
                Err(io::Error::last_os_error())
            } else {
                Ok(read as usize)
            }
        });
        match read {
            Ok(Ok(0)) => return,
            Ok(Ok(read)) => {
                if buffer[..read].contains(&0x03) {
                    cancellation.cancel();
                    return;
                }
            }
            Ok(Err(error)) if error.kind() == io::ErrorKind::Interrupted => {}
            Ok(Err(error)) => {
                tracing::debug!(%error, "raw terminal Ctrl+C monitor stopped");
                return;
            }
            Err(_would_block) => {}
        }
    }
}

#[cfg(unix)]
fn read_terminal_mode(fd: RawFd) -> io::Result<libc::termios> {
    let mut mode = std::mem::MaybeUninit::uninit();
    if unsafe { libc::tcgetattr(fd, mode.as_mut_ptr()) } == -1 {
        return Err(io::Error::last_os_error());
    }
    Ok(unsafe { mode.assume_init() })
}

#[cfg(unix)]
fn write_terminal_mode(fd: RawFd, mode: &libc::termios) -> io::Result<()> {
    if unsafe { libc::tcsetattr(fd, libc::TCSANOW, mode) } == -1 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::os::fd::RawFd;

    struct TestPty {
        master: RawFd,
        slave: RawFd,
    }

    impl TestPty {
        fn open() -> Self {
            let mut master = -1;
            let mut slave = -1;
            let result = unsafe {
                libc::openpty(
                    &mut master,
                    &mut slave,
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                )
            };
            assert_eq!(result, 0, "openpty failed: {}", io::Error::last_os_error());
            Self { master, slave }
        }
    }

    impl Drop for TestPty {
        fn drop(&mut self) {
            unsafe {
                libc::close(self.master);
                libc::close(self.slave);
            }
        }
    }

    #[test]
    fn enables_and_restores_interrupt_generation_for_raw_terminal() {
        let pty = TestPty::open();
        let mut raw = read_terminal_mode(pty.slave).expect("read pseudo-terminal mode");
        raw.c_lflag &= !libc::ISIG;
        write_terminal_mode(pty.slave, &raw).expect("disable pseudo-terminal interrupts");

        let guard = InterruptSignalGuard::enable_for_fd(pty.slave)
            .expect("enable pseudo-terminal interrupts");
        let enabled = read_terminal_mode(pty.slave).expect("read enabled terminal mode");
        assert_eq!(enabled.c_lflag, raw.c_lflag | libc::ISIG);
        assert_eq!(enabled.c_iflag, raw.c_iflag);
        assert_eq!(enabled.c_oflag, raw.c_oflag);
        assert_eq!(enabled.c_cflag, raw.c_cflag);
        assert_eq!(enabled.c_cc, raw.c_cc);
        assert_eq!(unsafe { libc::cfgetispeed(&enabled) }, unsafe {
            libc::cfgetispeed(&raw)
        });
        assert_eq!(unsafe { libc::cfgetospeed(&enabled) }, unsafe {
            libc::cfgetospeed(&raw)
        });

        drop(guard);
        let restored = read_terminal_mode(pty.slave).expect("read restored terminal mode");
        assert_eq!(restored.c_lflag, raw.c_lflag);
        assert_eq!(restored.c_iflag, raw.c_iflag);
        assert_eq!(restored.c_oflag, raw.c_oflag);
        assert_eq!(restored.c_cflag, raw.c_cflag);
        assert_eq!(restored.c_cc, raw.c_cc);
        assert_eq!(unsafe { libc::cfgetispeed(&restored) }, unsafe {
            libc::cfgetispeed(&raw)
        });
        assert_eq!(unsafe { libc::cfgetospeed(&restored) }, unsafe {
            libc::cfgetospeed(&raw)
        });
    }

    #[test]
    fn non_terminal_file_descriptor_is_a_no_op() {
        let mut fds = [-1; 2];
        assert_eq!(unsafe { libc::pipe(fds.as_mut_ptr()) }, 0);
        let guard = InterruptSignalGuard::enable_for_fd(fds[0])
            .expect("non-terminal descriptors should not fail");
        assert!(guard.terminal.is_none());
        unsafe {
            libc::close(fds[0]);
            libc::close(fds[1]);
        }
    }
}
