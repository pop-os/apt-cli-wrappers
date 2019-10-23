use apt_cli_wrappers::installed;
use std::env::args;

fn main() {
    let packages: Vec<String> = args().skip(1).collect();
    for package in installed(&mut String::new(), &packages) {
        println!("{} is installed", package);
    }
}
