use clap::{App, Arg};
use serde::Deserialize;
use std::collections::HashMap;
use std::{env, fs, path::PathBuf};

static DEFAULT_CONFIG_NAME: &str = "instance_config.toml";
static SETTINGS_DEFAULT_BEHAVIOR: Behavior = Behavior::Fail;

// TODO: make a collision_mode type that can be only 3 different values.

#[derive(Deserialize, Debug, Clone)]
pub struct Template {
    path: String,           // the path at which the template can be found
    call_name: String,      // what the tool should call to instance a template
    rename: Option<String>, // what the template should be renamed to
    behavior: Option<Behavior>, // whether the template should fail, append, or replace
                            // existing files with the same extension. default is
                            // "fail" but this can be configured per template.
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "lowercase")]
pub enum Behavior {
    Fail,
    Append,
    Overwrite,
}

#[derive(Deserialize)]
pub struct Settings {
    default_behavior: Option<Behavior>,
}

#[derive(Deserialize)]
pub struct Config {
    settings: Settings,
    templates: HashMap<String, Template>,
}

fn main() -> anyhow::Result<()> {
    // instance cli interface
    let matches = App::new("instance")
        .version("0.1.0")
        .author("bootstrap-prime <bootstrap.prime@gmail.com>")
        .about("A fast, efficient template generator and project initialization system written in Rust")
        .arg(Arg::with_name("template")
             .help("Specifies which template to use")
             .index(1))
        .arg(Arg::with_name("list")
             .short("l")
             .long("list")
             .help("List available templates"))
        .arg(Arg::with_name("config")
             .short("c")
             .long("config")
             .value_name("FILE")
             .help("Sets a config file")
             .takes_value(true))
        .get_matches();

    let current_dir = env::current_dir()?;

    let template_dir_path = {
        if matches.is_present("config") {
            matches.value_of("config").unwrap().to_string()
        } else {
            env::var("INSTANCE_TEMPLATE_DIR")?
        }
    };

    // read data from config file
    let file_input: Config = toml::from_str(&{
        let config_path: PathBuf = [&template_dir_path, &DEFAULT_CONFIG_NAME.to_string()]
            .iter()
            .collect();
        fs::read_to_string(&config_path)?
    })?;
    let template_data: Vec<Template> = file_input
        .templates
        .into_iter()
        .map(|(_id, templ)| templ)
        .collect();
    let _settings_data: Settings = file_input.settings;

    // validate template: ensure all template definitions are valid
    let invalid_templates = validate_template(&template_dir_path, &template_data);
    if !invalid_templates.is_empty() {
        println!(
            "There are {} invalid templates: \n{}\nFix paths relative to your configuration file.",
            invalid_templates.len(),
            {
                &invalid_templates
                    .iter()
                    .map(|element| element.call_name.to_string())
                    .collect::<Vec<String>>()
                    .join("\n")
            }
        );
        std::process::exit(-1);
    }

    // list elements provided by config
    if matches.is_present("list") {
        let elements_iter = template_data
            .iter()
            .map(|data| format!("  {}", &data.call_name))
            .collect::<Vec<String>>()
            .join("\n");
        println!("possible templates: \n{}", elements_iter);
    }

    // copy template to working directory
    if let Some(o) = matches.value_of("template") {
        //println!("{}", current_dir.into_os_string().into_string().unwrap());
        let get_val = template_data
            .iter()
            .find(|&element| element.call_name.eq_ignore_ascii_case(o));

        match get_val {
            None => println!("name did not match any template on record."),
            Some(element) => {
                let file_path_source: PathBuf = [&template_dir_path, &element.path].iter().collect();
                let file_path_destin: PathBuf = [&current_dir, &PathBuf::from(element.rename.as_ref().unwrap_or(&element.path.clone()))].iter().collect();
                println!(
                    "template: {} at {:?}",
                    &element.call_name, &file_path_source
                );
                fs::copy(file_path_source, file_path_destin)?;
            }
        };
    }

    Ok(())
}

// validate config, check to see if all templates exist
// TODO: use error handling with this validator.
fn validate_template<'a>(root_data: &'a str, template_data: &'a [Template]) -> Vec<&'a Template> {
    template_data
        .iter()
        .filter(|element| {
            ![root_data, &element.path]
                .iter()
                .collect::<PathBuf>()
                .exists()
        })
        .collect::<Vec<&Template>>()
}

//fn copy_val_to_
