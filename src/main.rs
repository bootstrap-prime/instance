use serde::Deserialize;
use std::fs;
use toml::Value;
use std::collections::HashMap;

static DEFAULT_CONFIG_LOCATION: &'static str = "templates/instance_config.toml";

// TODO: make a collision_mode type that can be only 3 different values.

// #[derive(Deserialize)]
// pub struct Template {
//     path: String,          // the path at which the template can be found
//     name: Option<String>,           // what the template should be renamed to
//     collision_mode: Option<String>, // whether the template should fail,
//     // append, or replace existing files with the same extension. default is "fail" but can
//     // be configured per template.
// }

#[derive(Deserialize)]
pub struct Settings {
    default_collision_mode: Option<String>,
}

#[derive(Deserialize)]
pub struct Config {
    settings: Settings,
    templates: Template,
}

#[derive(Deserialize)]
pub struct Template {
    #[serde(flatten)]
    everything_else: HashMap<String, Value>,
}

// pub type collision_mode: String;
// pub struct collision_mode<String>;

fn main() -> anyhow::Result<()> {
    let file_input = fs::read_to_string(DEFAULT_CONFIG_LOCATION)?;

    let value: Config = toml::from_str(&file_input).unwrap();

    println!("{}", value.templates.everything_else["nix"]["default"].as_str().unwrap_or("failed"));

    Ok(())
}
