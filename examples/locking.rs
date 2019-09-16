use apt_cli_wrappers::*;

fn main() {
    wait_for_apt_locks(3000, || println!("Now ready to use apt"));
}
