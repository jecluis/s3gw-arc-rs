// Copyright 2023 SUSE LLC
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use std::fmt::Display;

#[derive(Clone)]
pub struct Version {
    pub version: String,
    pub major: u64,
    pub minor: u64,
    pub patch: Option<u64>,
    pub rc: Option<u64>,
}

pub struct ReleaseVersion {
    pub release_version: Version,
    pub versions: Vec<Version>,
}

impl PartialEq for Version {
    fn eq(&self, other: &Self) -> bool {
        self.get_version_id() == other.get_version_id()
    }
}
impl Eq for Version {}

impl Display for Version {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}", self.major, self.minor)?;
        if let Some(v) = self.patch {
            write!(f, ".{}", v)?;
        }
        if let Some(v) = self.rc {
            write!(f, "-rc{}", v)?;
        }
        Ok(())
    }
}

impl Version {
    pub fn from_str(value: &String) -> Result<Version, ()> {
        let pattern = r"^v?((\d+)\.(\d+)(?:\.(\d+)(?:-rc(\d+))?)?)$";
        let re = match regex::Regex::new(&pattern) {
            Ok(v) => v,
            Err(e) => {
                log::error!("Error creating regex for pattern '{}': {}", pattern, e);
                return Err(());
            }
        };

        let m = match re.captures(&value) {
            Some(v) => v,
            None => {
                log::debug!("Error matching pattern '{}' to '{}'", pattern, value);
                return Err(());
            }
        };
        assert_eq!(m.len(), 6);

        log::trace!("m: len = {}", m.len());

        for c in m.iter() {
            log::trace!(
                "capture: {}",
                match c {
                    Some(v) => v.as_str(),
                    None => "N/A",
                }
            );
        }

        let version = m.get(1).expect("version must not be empty!").as_str();

        let major: u64 = m
            .get(2)
            .expect("major version should not be empty!")
            .as_str()
            .parse()
            .unwrap();
        let minor: u64 = m
            .get(3)
            .expect("minor version should not be empty!")
            .as_str()
            .parse()
            .unwrap();
        let mut patch: Option<u64> = None;
        let mut rc: Option<u64> = None;

        if let Some(v) = m.get(4) {
            patch = Some(v.as_str().parse::<u64>().unwrap());
        }
        if let Some(v) = m.get(5) {
            rc = Some(v.as_str().parse::<u64>().unwrap());
        }

        Ok(Version {
            version: String::from(version),
            major,
            minor,
            patch,
            rc,
        })
    }

    pub fn get_version_id(self: &Self) -> u64 {
        let mut patch: u64 = 999;
        let mut rc: u64 = 999;

        if let Some(v) = self.patch {
            patch = v;
        }
        if let Some(v) = self.rc {
            rc = v;
        }

        self.major * 10_u64.pow(9) + self.minor * 10_u64.pow(6) + patch * 10_u64.pow(3) + rc
    }

    pub fn get_release_version_str(self: &Self) -> String {
        format!("{}.{}", self.major, self.minor)
    }

    pub fn get_version_str(self: &Self) -> String {
        let p = match self.patch {
            Some(v) => format!(".{}", v),
            None => String::new(),
        };
        let rc = match self.rc {
            Some(v) => format!("-rc{}", v),
            None => String::new(),
        };
        format!("{}.{}{}{}", self.major, self.minor, p, rc,)
    }
}
