use std::collections::HashMap;
use std::fmt::{self, Display, Formatter};
use std::str::FromStr;
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AptUpgradeEvent {
    Processing { package: Box<str> },
    Progress { percent: u8 },
    SettingUp { package: Box<str> },
    Unpacking { package: Box<str>, version: Box<str>, over: Box<str> },
}

impl AptUpgradeEvent {
    pub fn into_dbus_map(self) -> HashMap<&'static str, String> {
        let mut map = HashMap::new();

        match self {
            AptUpgradeEvent::Processing { package } => {
                map.insert("processing_package", package.into());
            }
            AptUpgradeEvent::Progress { percent } => {
                map.insert("percent", percent.to_string());
            }
            AptUpgradeEvent::SettingUp { package } => {
                map.insert("setting_up", package.into());
            }
            AptUpgradeEvent::Unpacking { package, version, over } => {
                map.insert("unpacking", package.into());
                map.insert("version", version.into());
                map.insert("over", over.into());
            }
        }

        map
    }

    pub fn from_dbus_map<K: AsRef<str>, V: AsRef<str> + Into<Box<str>>>(
        mut map: impl Iterator<Item = (K, V)>,
    ) -> Result<Self, ()> {
        use self::AptUpgradeEvent::*;

        let (key, value) = match map.next() {
            Some(value) => value,
            None => return Err(()),
        };

        let event = match key.as_ref() {
            "processing_package" => Processing { package: value.into() },
            "percent" => {
                let percent = value.as_ref().parse::<u8>().map_err(|_| ())?;
                Progress { percent }
            }
            "setting_up" => SettingUp { package: value.into() },
            "over" => match (map.next(), map.next()) {
                (Some((key1, value1)), Some((key2, value2))) => {
                    let over = value.into();
                    let value1 = value1.into();
                    let value2 = value2.into();
                    match (key1.as_ref(), key2.as_ref()) {
                        ("version", "unpacking") => {
                            Unpacking { package: value2, version: value1, over }
                        }
                        ("unpacking", "version") => {
                            Unpacking { package: value1, version: value2, over }
                        }
                        _ => return Err(()),
                    }
                }
                _ => return Err(()),
            },
            _ => return Err(()),
        };

        Ok(event)
    }
}

impl Display for AptUpgradeEvent {
    fn fmt(&self, fmt: &mut Formatter) -> fmt::Result {
        match self {
            AptUpgradeEvent::Processing { package } => {
                write!(fmt, "processing triggers for {}", package)
            }
            AptUpgradeEvent::Progress { percent } => write!(fmt, "progress: [{:>3}%]", percent),
            AptUpgradeEvent::SettingUp { package } => write!(fmt, "setting up {}", package),
            AptUpgradeEvent::Unpacking { package, version, over } => {
                write!(fmt, "unpacking {} ({}) over ({})", package, version, over)
            }
        }
    }
}

// TODO: Unit test this
impl FromStr for AptUpgradeEvent {
    type Err = ();

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        if input.starts_with("Progress: [") {
            let (_, progress) = input.split_at(11);
            let progress = progress.trim_right();
            if progress.len() == 5 {
                if let Ok(percent) = progress[..progress.len() - 2].trim_left().parse::<u8>() {
                    return Ok(AptUpgradeEvent::Progress { percent });
                }
            }
        } else if input.starts_with("Processing triggers for ") {
            let (_, input) = input.split_at(24);
            if let Some(package) = input.split_whitespace().next() {
                return Ok(AptUpgradeEvent::Processing { package: package.into() });
            }
        } else if input.starts_with("Setting up ") {
            let (_, input) = input.split_at(11);
            if let Some(package) = input.split_whitespace().next() {
                return Ok(AptUpgradeEvent::SettingUp { package: package.into() });
            }
        } else if input.starts_with("Unpacking ") {
            let (_, input) = input.split_at(10);
            let mut fields = input.split_whitespace();
            if let (Some(package), Some(version), Some(over)) =
                (fields.next(), fields.next(), fields.nth(1))
            {
                if version.len() > 2 && over.len() > 2 {
                    return Ok(AptUpgradeEvent::Unpacking {
                        package: package.into(),
                        version: version[1..version.len() - 1].into(),
                        over: over[1..over.len() - 1].into(),
                    });
                }
            }
        }

        Err(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn apt_upgrade_event_progress() {
        assert_eq!(
            AptUpgradeEvent::Progress { percent: 1 },
            "Progress: [  1%]".parse::<AptUpgradeEvent>().unwrap()
        );

        assert_eq!(
            AptUpgradeEvent::Progress { percent: 25 },
            "Progress: [ 25%] ".parse::<AptUpgradeEvent>().unwrap()
        );

        assert_eq!(
            AptUpgradeEvent::Progress { percent: 100 },
            "Progress: [100%]".parse::<AptUpgradeEvent>().unwrap()
        );
    }
}
