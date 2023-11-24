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

use handlebars::Handlebars;
use std::{collections::HashMap, path::PathBuf};

use crate::{
    release::{errors::ReleaseResult, Release},
    version::Version,
};

pub fn announce(
    _release: &mut Release,
    version: &Version,
    _outfile: &Option<PathBuf>,
) -> ReleaseResult<()> {
    let mut hb = Handlebars::new();
    let tmpl_str = "
The s3gw team is {{mood}} to announce the release of S3 Gateway v{{version}}!
This release includes a few exciting changes, most notably:

{{changelog}}
    
Get the container images from:
    
    quay.io/s3gw/s3gw:v{{version}}
    quay.io/s3gw/s3gw-ui:v{{version}}
        
or through our Helm Chart at https://artifacthub.io/packages/helm/s3gw/s3gw/{{version}}

For more information, check our changelog at

    https://s3gw-docs.readthedocs.io/en/main/release-notes/s3gw-v{{version}}/
";

    hb.register_template_string("announcement", tmpl_str)
        .unwrap();
    let mut data = HashMap::new();
    data.insert("mood", String::from("excited"));
    data.insert("version", version.to_string());
    data.insert("changelog", String::from("things that changed"));

    println!("{}", hb.render("announcement", &data).unwrap());

    Ok(())
}
