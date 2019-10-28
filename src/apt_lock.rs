use std::{path::Path, thread, time::Duration};

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
