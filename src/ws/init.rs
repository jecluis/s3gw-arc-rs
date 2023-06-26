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

use inquire::{error::InquireResult, required, Confirm, Text};

pub struct PromptValues {
    pub name: String,
    pub email: String,
    pub signing_key: String,
}

pub async fn prompt() -> Result<PromptValues, ()> {
    let name = match Text::new("User Name:").with_validator(required!()).prompt() {
        Ok(v) => v,
        Err(_) => return Err(()),
    };
    let email = match Text::new("User email:")
        .with_validator(|v: &str| {
            let re = regex::Regex::new(r"^[\w_\-.]+@[\w\-_.]+$").unwrap();
            if re.is_match(&v) {
                return Ok(inquire::validator::Validation::Valid);
            }
            Ok(inquire::validator::Validation::Invalid(
                "must be an email address".into(),
            ))
        })
        .prompt()
    {
        Ok(v) => v,
        Err(_) => return Err(()),
    };
    let signing_key = match Text::new("Signing key:")
        .with_validator(required!())
        .prompt()
    {
        Ok(v) => v,
        Err(_) => return Err(()),
    };

    let answer = match Confirm::new("Do you want to setup custom repositories?")
        .with_default(false)
        .prompt()
    {
        Ok(v) => v,
        Err(_) => return Err(()),
    };

    Ok(PromptValues {
        name,
        email,
        signing_key,
    })
}
