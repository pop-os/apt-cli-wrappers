use apt_cli_wrappers::predepends_of;
use std::env;

fn main() {
    let predepend_on = env::args().skip(1).next().unwrap();
    for package in predepends_of(&mut String::new(), &predepend_on).unwrap() {
        println!("{} is a predepends of {}", package, predepend_on);
    }
}
