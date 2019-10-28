use std::{
    io,
    os::unix::process::ExitStatusExt,
    process::{Command, Stdio},
};

pub fn check_output<F: FnOnce(&mut Command) -> &mut Command>(
    cmd: &str,
    func: F,
) -> io::Result<String> {
    let output = func(&mut Command::new(cmd))
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .output()?;

    if output.status.success() {
        String::from_utf8(output.stdout).map_err(|_| {
            io::Error::new(io::ErrorKind::Other, format!("{} output was not UTF-8", cmd))
        })
    } else {
        let source = match output.status.code() {
            Some(code) => io::Error::new(
                io::ErrorKind::Other,
                format!("{} exited with status of {}", cmd, code),
            ),
            None => match output.status.signal() {
                Some(signal) => io::Error::new(
                    io::ErrorKind::Other,
                    format!("{} terminated with signal {}", cmd, signal),
                ),
                None => io::Error::new(io::ErrorKind::Other, format!("{} status is unknown", cmd)),
            },
        };

        Err(source)
    }
}
