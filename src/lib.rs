mod upgrade_event;

pub use self::upgrade_event::AptUpgradeEvent;
use exit_status_ext::ExitStatusExt;

use std::{
    fs::File,
    io::{self, BufRead, BufReader},
    os::unix::io::{FromRawFd, IntoRawFd},
    path::Path,
    process::{Command, Stdio},
    thread,
    time::Duration,
};

/// Execute the apt command non-interactively, using whichever additional arguments are provided.
pub fn apt_noninteractive<F: FnMut(&mut Command) -> &mut Command>(mut func: F) -> io::Result<()> {
    func(
        Command::new("apt-get")
            .env("DEBIAN_FRONTEND", "noninteractive")
            .args(&["-y", "--allow-downgrades"]),
    )
    .status()
    .and_then(ExitStatusExt::as_result)
}

/// Same as `apt_noninteractive`, but also has a callback for handling lines of output.
pub fn apt_noninteractive_callback<F: FnMut(&mut Command) -> &mut Command, C: Fn(&str)>(
    mut func: F,
    callback: C,
) -> io::Result<()> {
    let mut child = func(
        Command::new("apt-get")
            .env("DEBIAN_FRONTEND", "noninteractive")
            .env("LANG", "C")
            .args(&["-y", "--allow-downgrades"]),
    )
    .stdout(Stdio::piped())
    .spawn()?;

    let mut stdout_buffer = String::new();
    let mut stdout = child.stdout.take().map(non_blocking).map(BufReader::new);

    loop {
        thread::sleep(Duration::from_millis(16));
        match child.try_wait()? {
            Some(status) => return status.as_result(),
            None => {
                if let Some(ref mut stdout) = stdout {
                    let _ = non_blocking_line_reading(stdout, &mut stdout_buffer, &callback);
                }
            }
        }
    }
}

// apt-autoremove -y
pub fn apt_autoremove<L: FnMut(bool)>(readiness: L) -> io::Result<()> {
    wait_for_apt_locks(3000, readiness, || apt_noninteractive(|cmd| cmd.arg("autoremove")))
}

/// apt-get -y --allow-downgrades install
pub fn apt_install<L: FnMut(bool)>(packages: &[&str], readiness: L) -> io::Result<()> {
    wait_for_apt_locks(3000, readiness, || {
        apt_noninteractive(move |cmd| cmd.arg("install").args(packages))
    })
}

pub fn apt_install_fix_broken<L: FnMut(bool)>(readiness: L) -> io::Result<()> {
    wait_for_apt_locks(3000, readiness, || {
        apt_noninteractive(move |cmd| cmd.args(&["install", "-f"]))
    })
}

/// apt-get -y --allow-downgrades purge
pub fn apt_purge<L: FnMut(bool)>(packages: &[&str], readiness: L) -> io::Result<()> {
    wait_for_apt_locks(3000, readiness, || {
        apt_noninteractive(move |cmd| cmd.arg("purge").args(packages))
    })
}

/// apt-get -y --allow-downgrades install --reinstall
pub fn apt_reinstall<L: FnMut(bool)>(packages: &[&str], readiness: L) -> io::Result<()> {
    wait_for_apt_locks(3000, readiness, || {
        apt_noninteractive(move |cmd| cmd.arg("install").arg("--reinstall").args(packages))
    })
}

/// apt-get remove --autoremove -y
pub fn apt_remove<L: FnMut(bool)>(packages: &[&str], readiness: L) -> io::Result<()> {
    wait_for_apt_locks(3000, readiness, || {
        apt_noninteractive(move |cmd| cmd.arg("remove").arg("--autoremove").args(packages))
    })
}

/// apt-get -y --allow-downgrades full-upgrade
pub fn apt_update<L: FnMut(bool)>(readiness: L) -> io::Result<()> {
    wait_for_apt_locks(3000, readiness, || apt_noninteractive(|cmd| cmd.arg("update")))
}

/// apt-get -y --allow-downgrades full-upgrade
pub fn apt_upgrade<C: Fn(AptUpgradeEvent)>(callback: C) -> io::Result<()> {
    let callback = &callback;
    let readiness = |ready: bool| {
        if !ready {
            callback(AptUpgradeEvent::WaitingOnLock)
        }
    };
    wait_for_apt_locks(3000, readiness, || {
        apt_noninteractive_callback(
            |cmd| cmd.args(&["--show-progress", "full-upgrade"]),
            move |line| {
                if let Ok(event) = line.parse::<AptUpgradeEvent>() {
                    callback(event);
                }
            },
        )
    })
}

/// dpkg --configure -a
pub fn dpkg_configure_all<L: FnMut(bool)>(readiness: L) -> io::Result<()> {
    // TODO: progress callback support.
    wait_for_apt_locks(3000, readiness, || {
        Command::new("dpkg")
            .args(&["--configure", "-a"])
            .status()
            .and_then(ExitStatusExt::as_result)
    })
}

pub fn apt_hold(package: &str) -> io::Result<()> {
    Command::new("apt-mark").args(&["hold", package]).status().and_then(ExitStatusExt::as_result)
}

pub fn apt_unhold(package: &str) -> io::Result<()> {
    Command::new("apt-mark").args(&["unhold", package]).status().and_then(ExitStatusExt::as_result)
}

fn non_blocking<F: IntoRawFd>(fd: F) -> File {
    let fd = fd.into_raw_fd();
    unsafe {
        let flags = libc::fcntl(fd, libc::F_GETFL, 0);
        libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK);
        File::from_raw_fd(fd)
    }
}

fn non_blocking_line_reading<B: BufRead, F: Fn(&str)>(
    reader: &mut B,
    buffer: &mut String,
    callback: F,
) -> io::Result<()> {
    loop {
        match reader.read_line(buffer) {
            Ok(0) => break,
            Ok(_read) => {
                callback(&buffer);
                buffer.clear();
            }
            Err(ref why) if why.kind() == io::ErrorKind::WouldBlock => break,
            Err(why) => {
                buffer.clear();
                return Err(why);
            }
        }
    }

    Ok(())
}

const LISTS_LOCK: &str = "/var/lib/apt/lists/lock";
const DPKG_LOCK: &str = "/var/lib/dpkg/lock";

pub fn wait_for_apt_locks<R, L: FnMut(bool), F: FnOnce() -> R>(
    delay: u64,
    mut readiness: L,
    func: F,
) -> R {
    let paths = &[Path::new(DPKG_LOCK), Path::new(LISTS_LOCK)];

    let waiting = lock_found(paths);

    if waiting {
        readiness(false);
        thread::sleep(Duration::from_millis(delay));
        while lock_found(paths) {
            thread::sleep(Duration::from_millis(delay));
        }
    }

    readiness(true);
    func()
}

fn lock_found(paths: &[&Path]) -> bool {
    for proc in procfs::all_processes() {
        if let Ok(fdinfos) = proc.fd() {
            for fdinfo in fdinfos {
                if let procfs::FDTarget::Path(path) = fdinfo.target {
                    if paths.into_iter().any(|&p| &*path == p) {
                        return true;
                    }
                }
            }
        }
    }

    false
}
