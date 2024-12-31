// SPDX-FileCopyrightText: Copyright © 2020-2024 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0

use std::io;

use fs_err as fs;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use thiserror::Error;
use tui::Styled;

#[derive(Serialize)]
struct MonitoringTemplate {
    releases: Releases,
    security: Security,
}

#[derive(Serialize)]
struct Releases {
    id: Option<u32>,
}

#[derive(Serialize)]
struct Security {
    cpe: Option<Vec<Cpe>>,
}

#[derive(Serialize)]
struct Cpe {
    vendor: Option<String>,
    product: Option<String>,
}

#[derive(Debug, Deserialize)]
struct Item {
    id: u32,
    name: String,
}

#[derive(Debug, Deserialize)]
struct Response {
    items: Vec<Item>,
    total_items: u32,
}

#[derive(Serialize)]
pub struct Monitoring {
    name: String,
}

impl Monitoring {
    pub fn new(name: String) -> Self {
        Self { name }
    }

    pub fn run(&self) -> Result<(), Error> {
        let client = self.create_reqwest_client();

        let id = self.find_monitoring_id(&self.name, &client)?;
        let cpes = self.find_security_cpe(&self.name, &client)?;

        self.write_monitoring(id, cpes)?;

        Ok(())
    }

    fn create_reqwest_client(&self) -> reqwest::blocking::Client {
        reqwest::blocking::Client::new()
    }

    fn find_monitoring_id(&self, name: &String, client: &reqwest::blocking::Client) -> Result<Option<u32>, Error> {
        let url = format!("https://release-monitoring.org/api/v2/projects/?name={}", name);

        let resp = client.get(&url).send()?;

        if !resp.status().is_success() {
            println!("Failed to get monitoring ID, error {}", resp.status());
        }

        let body: Response = resp.json()?;

        if body.total_items == 1 {
            if let Some(result) = body.items.first() {
                println!(
                    "{} | Matched id {} from {}",
                    "Monitoring".green(),
                    result.id,
                    result.name
                );
                Ok(Some(result.id))
            } else {
                Ok(None)
            }
        } else if body.total_items > 1 && body.total_items < 10 {
            println!("{} | Multiple potential IDs matched, find the correct ID for the project at https://release-monitoring.org/", "Warning".yellow());
            for i in body.items {
                println!(
                    "ID {} Name {} URL https://release-monitoring.org/project/{}/",
                    i.id, i.name, i.id
                );
            }
            println!();
            Ok(None)
        } else {
            println!(
                "{} | Find the correct ID for the project at https://release-monitoring.org/",
                "Warning".yellow()
            );
            Ok(None)
        }
    }

    fn find_security_cpe(&self, name: &String, client: &reqwest::blocking::Client) -> Result<Option<Vec<Cpe>>, Error> {
        let url = "https://cpe-guesser.cve-search.org/search";

        let mut query = HashMap::new();
        query.insert("query", [name]);

        let resp = client.post(url).json(&query).send()?;

        if !resp.status().is_success() {
            println!("Failed to get monitoring CPE, error {}", resp.status());
        }

        let json: Vec<Vec<Value>> = serde_json::from_str(&resp.text()?).unwrap_or_default();

        // Extract CPEs into a Option<Vec<CPE>>
        let cpes: Option<Vec<Cpe>> = json
            .iter()
            .map(|item| {
                if let Some(Value::String(cpe_string)) = item.get(1) {
                    // Split the CPE string and extract the desired parts
                    let parts: Vec<&str> = cpe_string.split(':').collect();
                    if parts.len() > 4 {
                        let vendor = parts[3].to_string();
                        let product = parts[4].to_string();
                        println!(
                            "{} | Matched CPE Vendor: {} Product: {}",
                            "Security".green(),
                            vendor,
                            product
                        );
                        return Some(Cpe {
                            vendor: Some(vendor),
                            product: Some(product),
                        });
                    }
                }
                None
            })
            .collect();
        println!();

        Ok(cpes)
    }

    fn write_monitoring(&self, id: Option<u32>, cpes: Option<Vec<Cpe>>) -> Result<String, Error> {
        // We may not have matched any ID or CPE which is fine
        // Unwrap the default value then mangle it into a YAML ~ (null) value
        let id_value = id.unwrap_or_default();
        let cpe_value = cpes.unwrap_or_default();

        // fighting the borrow checker, cba
        let mut empty_cpe = false;
        if cpe_value.is_empty() {
            empty_cpe = true;
        }

        if cpe_value.len() > 1 {
            println!(
                "{} | Multiple CPEs matched, please verify and remove any superfluous",
                "Warning".yellow()
            );
        }

        let monitoring_template = MonitoringTemplate {
            releases: Releases { id: Some(id_value) },
            security: Security { cpe: Some(cpe_value) },
        };

        let mut yaml_string = serde_yaml::to_string(&monitoring_template).expect("Failed to serialize to YAML");

        if id_value == 0 {
            let id_string = "id: 0";
            let id_marker = yaml_string.find(id_string).expect("releases id marker not found");
            yaml_string = yaml_string.replace(id_string, "id: ~");
            let id_help_text = " # https://release-monitoring.org/ and use the numeric id in the url of project";
            yaml_string.insert_str(id_marker + id_string.len(), id_help_text);
        }

        if empty_cpe {
            let cpe_string = "cpe: []";
            let cpe_marker = yaml_string.find(cpe_string).expect("security cpe marker not found");
            yaml_string = yaml_string.replace(cpe_string, "cpe: ~");
            let cpe_help_text = format!(
                " # Last checked {}",
                chrono::Local::now().date_naive().format("%Y-%m-%d")
            );
            yaml_string.insert_str(cpe_marker + cpe_string.len() - 1, &cpe_help_text);
        }

        let output = "monitoring.yaml";
        fs::write(output, yaml_string.as_bytes())?;
        println!("Wrote {:?} file", output);
        Ok(yaml_string)
    }
}

#[derive(Debug, Error)]
pub enum Error {
    #[error("io")]
    Io(#[from] io::Error),
    #[error("statuscode")]
    StatusCode(#[from] reqwest::Error),
}
