use crate::apt_cache;
use std::{io, str::Lines};

/// Fetches all packages which have a predepends on the given package.
pub fn predepends_of<'a>(
    buffer: &'a mut String,
    package: &'a str,
) -> io::Result<PreDependsIter<'a>> {
    let output = apt_cache("rdepends", &[package], |_| {})?;
    let depends = output.lines().skip(2).map(|x| x.trim_start()).collect::<Vec<&str>>();

    *buffer = apt_cache("depends", &depends, |_| {})?;
    PreDependsIter::new(buffer.as_str(), package)
}

pub struct PreDependsIter<'a> {
    lines: Lines<'a>,
    predepend: &'a str,
    active: &'a str,
}

impl<'a> PreDependsIter<'a> {
    pub fn new(output: &'a str, predepend: &'a str) -> io::Result<Self> {
        let mut lines = output.lines();

        let active = lines.next().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                "expected the first line of the output of apt-cache depends to be a package name",
            )
        })?;

        Ok(Self { lines, predepend, active: active.trim() })
    }
}

impl<'a> Iterator for PreDependsIter<'a> {
    type Item = &'a str;

    fn next(&mut self) -> Option<Self::Item> {
        let mut found = false;
        while let Some(line) = self.lines.next() {
            if !line.starts_with(' ') {
                let prev = self.active;
                self.active = line.trim();
                if found {
                    return Some(prev);
                }
            } else if !found && line.starts_with("  PreDepends: ") && &line[14..] == self.predepend
            {
                found = true;
            }
        }

        None
    }
}
