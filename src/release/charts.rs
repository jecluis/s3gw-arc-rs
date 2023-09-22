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

use std::io::BufRead;
use std::io::BufReader;
use std::io::BufWriter;
use std::io::Write;
use std::path::PathBuf;

use crate::{boomln, version::Version, ws::repository::Repository};

use super::errors::ChartsError;

pub fn update_charts(repo: &Repository, version: &Version) -> Result<(), ChartsError> {
    let chart_path_rel = PathBuf::from("charts/s3gw/Chart.yaml");
    let chart_path = repo.path.join(&chart_path_rel);
    if !chart_path.exists() {
        return Err(ChartsError::DoesNotExistError);
    }

    if let Err(err) = chart_update_version(&chart_path, &version) {
        boomln!(format!("Unable to update chart version: {}", err));
        return Err(err);
    }

    if let Err(()) = repo.stage_paths(&vec![chart_path_rel]) {
        boomln!("Unable to stage chart changes!");
        return Err(ChartsError::UnknownError);
    }

    match std::process::Command::new("git")
        .args([
            "-C",
            repo.path.to_str().unwrap(),
            "commit",
            "--gpg-sign",
            "--signoff",
            "-m",
            format!("Update charts to version {}", version).as_str(),
        ])
        .status()
    {
        Ok(res) => {
            if !res.success() {
                boomln!(format!(
                    "Unable to commit chart update: {}",
                    res.code().unwrap()
                ));
                return Err(ChartsError::UnknownError);
            }
        }
        Err(err) => {
            boomln!(format!("Error committing chart update: {}", err));
            return Err(ChartsError::UnknownError);
        }
    };

    Ok(())
}

fn chart_update_version(chart_path: &PathBuf, version: &Version) -> Result<(), ChartsError> {
    let f = match std::fs::File::open(&chart_path) {
        Ok(f) => f,
        Err(err) => {
            boomln!(format!(
                "Unable to open chart file at '{}': {}",
                chart_path.display(),
                err
            ));
            return Err(ChartsError::UnknownError);
        }
    };

    let mut tmp_chart_path = chart_path.clone();
    tmp_chart_path.set_extension("yaml.tmp");
    let tmp_chart = match std::fs::File::options()
        .create_new(true)
        .write(true)
        .open(&tmp_chart_path)
    {
        Ok(f) => f,
        Err(err) => {
            boomln!(format!("Unable to open tmp chart file: {}", err));
            return Err(ChartsError::UnknownError);
        }
    };

    let version_re = regex::Regex::new(r"^version:[ ]+(.*)$").unwrap();

    let mut writer = BufWriter::new(tmp_chart);
    let reader = BufReader::new(f);
    for line_res in reader.lines() {
        let mut line = match line_res {
            Ok(s) => s,
            Err(err) => {
                boomln!(format!("Unable to obtain line from chart file: {}", err));
                return Err(ChartsError::UnknownError);
            }
        };

        if let Some(m) = version_re.captures(&line) {
            let cur_ver = match Version::from_str(&m[1].into()) {
                Ok(v) => v,
                Err(()) => {
                    boomln!("Unable to parse current charts version!");
                    return Err(ChartsError::UnknownError);
                }
            };
            log::debug!("chart version: cur {} next {}", cur_ver, version);
            line = format!("version: {}", version);
        }
        line.push('\n');
        match writer.write(line.as_bytes()) {
            Ok(_) => {}
            Err(err) => {
                boomln!(format!("Error writing to tmp charts file: {}", err));
                return Err(ChartsError::UnknownError);
            }
        };
    }

    if let Err(err) = std::fs::remove_file(&chart_path) {
        boomln!(format!(
            "Error removing charts file for replacement: {}",
            err
        ));
        return Err(ChartsError::UnknownError);
    }

    if let Err(err) = std::fs::rename(&tmp_chart_path, &chart_path) {
        boomln!(format!("Error renaming tmp charts file: {}", err));
        return Err(ChartsError::UnknownError);
    }

    Ok(())
}
