use apt_cli_wrappers::*;

fn main() {
    let on_lock = || println!("waiting for apt and dpkg lock to be available");
    let on_ready = || println!("Now ready to use apt");
    wait_for_apt_locks(3000, on_lock, on_ready);
}
