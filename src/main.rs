use clap::{App, Arg};
use serde::Deserialize;
use std::collections::HashMap;
use std::io::{self, Write};
use std::{env, path::PathBuf};
use std::{fs, fs::OpenOptions};

static DEFAULT_CONFIG_NAME: &str = "instance_config.toml";
static SETTINGS_DEFAULT_BEHAVIOR: Behavior = Behavior::Fail;
static DEFAULT_CONFIG_DIR: &str = "~/.templates";

#[derive(Deserialize, Debug, Clone)]
pub struct Template {
    path: String, // the path at which the template can be found
    #[serde(rename = "type")]
    ttype: TemplateType, // what the template is
    call_name: String, // what the tool should call to instance a template
    rename: Option<String>, // what the template should be renamed to
    behavior: Option<Behavior>, // whether the template should fail, append, or replace
                  // existing files with the same extension. default is
                  // "fail" but this can be configured per template.
}

// TODO: either impl Collection in TemplateType or Projects
#[derive(Deserialize, Debug, Clone)]
pub struct Project {
    templates: Vec<String>,
    call_name: String,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "lowercase")]
pub enum Behavior {
    Fail,
    Append,
    Overwrite,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "lowercase")]
pub enum TemplateType {
    Script,
    Template,
}

impl Default for TemplateType {
    fn default() -> Self {
        TemplateType::Template
    }
}

#[derive(Deserialize)]
pub struct Settings {
    default_behavior: Option<Behavior>,
}

// templates are required, and so are settings, but projects are optional.
#[derive(Deserialize)]
pub struct Config {
    settings: Settings,
    templates: HashMap<String, Template>,
    projects: Option<HashMap<String, Project>>,
}

fn main() -> anyhow::Result<()> {
    // instance cli interface
    let matches = App::new("instance")
        .version("0.1.0")
        .author("bootstrap-prime <bootstrap.prime@gmail.com>")
        .about("A fast, efficient template generator and project initialization system written in Rust")
        .arg(Arg::with_name("project")
             .help("specifies which project to use")
             .index(1))
        .arg(Arg::with_name("template")
             .short("t")
             .long("--template")
             .value_name("TEMPLATE")
             .help("Specifies which template to use")
             .takes_value(true))
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

    // figures out where the configuration is
    // order of priority is (most important to least) [given config -> env var -> default directory]
    let template_dir_path = match matches.value_of("config") {
        Some(value) => value.to_string(),
        None => match env::var("INSTANCE_TEMPLATE_DIR") {
            Ok(value) => value,
            Err(_) => DEFAULT_CONFIG_DIR.to_string(),
        },
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
    let project_data: Option<Vec<Project>> = file_input
        .projects
        .map(|e| e.into_iter().map(|(_id, templ)| templ).collect());
    let settings_data: Settings = file_input.settings;

    // validate template: ensure all template definitions are valid
    let invalid_templates = validate_template(&template_dir_path, &template_data);
    if !invalid_templates.is_empty() {
        println!(
            //TODO: if there's a nicer way to handle plurals in this, that would be amazing. but I haven't found one.
            "There {} {} invalid template{}: \n{}",
            match invalid_templates.len() {
                1 => "is",
                _ => "are",
            },
            invalid_templates.len(),
            match invalid_templates.len() {
                1 => "",
                _ => "s",
            },
            {
                &invalid_templates
                    .iter()
                    .map(|element| element.call_name.to_string())
                    .collect::<Vec<String>>()
                    .join("\n")
            },
        );
        std::process::exit(-1);
    }

    // validate projects: ensure all projects are valid
    match &project_data {
        Some(project_data) => {
            let invalid_projects = validate_project(&template_data, &project_data);
            if !invalid_projects.is_empty() {
                println!(
                    "There {} {} invalid project{}: \n{}",
                    match invalid_projects.len() {
                        1 => "is",
                        _ => "are",
                    },
                    invalid_projects.len(),
                    match invalid_projects.len() {
                        1 => "",
                        _ => "s",
                    },
                    {
                        &invalid_projects
                            .iter()
                            .map(|element| element.call_name.to_string())
                            .collect::<Vec<String>>()
                            .join("\n")
                    },
                );
                std::process::exit(-1);
            }
        }
        None => (),
    }

    // list elements provided by config
    if matches.is_present("list") {
        let owned_templates = template_data
            .iter()
            .map(|data| format!("  {}", &data.call_name))
            .collect::<Vec<String>>()
            .join("\n");

        println!("possible templates: \n{}", &owned_templates);

        match &project_data {
            Some(project_data) => {
                let owned_projects = project_data
                    .iter()
                    .map(|data| format!("  {}", &data.call_name))
                    .collect::<Vec<String>>()
                    .join("\n");

                println!("\npossible projects: \n{}", &owned_projects);
            }
            None => (),
        }
    }

    // instance all the templates in a project
    if let Some(o) = matches.value_of("project") {
        match project_data {
            Some(project_data) => {
                let get_val = project_data
                    .iter()
                    .find(|&element| element.call_name.eq_ignore_ascii_case(o));

                match get_val {
                    None => println!("name did not match any project on record."),
                    Some(element) => {
                        instantiate_project(
                            element,
                            &current_dir,
                            &template_dir_path,
                            &settings_data,
                            &template_data,
                        )?;
                    }
                };
            }
            None => println!("No projects included in configuration file."),
        }
    }

    // TODO: copy template to SPECIFIED directory instead of WORKING directory.
    // copy template to working directory
    if let Some(o) = matches.value_of("template") {
        let get_val = template_data
            .iter()
            .find(|&element| element.call_name.eq_ignore_ascii_case(o));

        match get_val {
            None => println!("name did not match any template on record."),
            Some(element) => {
                instantiate_template(element, &current_dir, &template_dir_path, &settings_data)?;
            }
        };
    }

    Ok(())
}

// check to see if all templates exist
// passes back a list of invalid templates
// TODO: use error handling with this validator to handle it as an error instead of like this
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

// check to see if all subtemplates exists and if the project's defined properly
// TODO: use error handling (again)
fn validate_project<'a>(
    template_data: &'a [Template],
    project_data: &'a [Project],
) -> Vec<&'a Project> {
    project_data
        .iter()
        // this is a list of INVALID projects. this filter removes VALID projects.
        .filter(|element| {
            for called_template in &element.templates {
                if !template_data
                    .iter()
                    .map(|e| &e.call_name)
                    .collect::<Vec<&String>>()
                    .contains(&called_template)
                {
                    // don't filter it out of the list of invalid projects if it's invalid
                    return true;
                }
            }
            // filter it out of the list if it's valid
            false
        })
        .collect::<Vec<&Project>>()
}

// instance a project (and all the templates in it)
fn instantiate_project(
    element: &Project,
    file_path_destin: &PathBuf,
    template_source_path: &str,
    settings_data: &Settings,
    templates: &Vec<Template>,
) -> anyhow::Result<()> {
    for template in &element.templates {
        let template_ref = &templates
            .iter()
            .find(|&element| element.call_name.eq_ignore_ascii_case(&template)).unwrap();

        instantiate_template(template_ref, &file_path_destin, &template_source_path, &settings_data)?;
    }

    Ok(())
}

// instance a template (whether script or file)
fn instantiate_template(
    element: &Template,
    base_path: &PathBuf,
    template_source_path: &str,
    settings_data: &Settings,
) -> anyhow::Result<()> {
    let file_path_source: PathBuf = [&template_source_path, &element.path.as_str()]
        .iter()
        .collect();

    let file_path_destin: PathBuf = [
        &base_path,
        &PathBuf::from(element.rename.as_ref().unwrap_or(&element.path.clone())),
    ]
    .iter()
    .collect();

    match element.ttype {
        TemplateType::Template => {
            // UI for easier debugging and user notification
            println!(
                "template: {} at {:?}",
                &element.call_name, &file_path_source
            );

            if file_path_destin.exists() {
                match &element.behavior.as_ref().unwrap_or_else(|| {
                    settings_data
                        .default_behavior
                        .as_ref()
                        .unwrap_or_else(|| &SETTINGS_DEFAULT_BEHAVIOR)
                }) {
                    // deal with file collisions (if a file is already present)
                    // multiple options: fail (just fail the whole thing), append (like for gitignore), overwrite (ignore the past and destroy it)

                    Behavior::Fail => {
                        println!("Error, file already exists and the setting for this template on conflict is failure.");
                        std::process::exit(-1)
                    }
                    Behavior::Append => {
                        let source_data = fs::read_to_string(&file_path_source)?;
                        let mut dest_file = OpenOptions::new()
                            .write(true)
                            .append(true)
                            .open(&file_path_destin)?;
                        dest_file.write_all(source_data.as_bytes())?;
                    }
                    Behavior::Overwrite => {
                        fs::remove_file(&file_path_destin)?;
                        fs::copy(&file_path_source, &file_path_destin)?;
                    }
                }
            } else {
                fs::copy(&file_path_source, &file_path_destin)?;
            }
        }
        TemplateType::Script => {
            // for things like cargo init and git init
            // automatically integrates with nix-shell shebangs for user convenience.
            println!("script: {} at {:?}", &element.call_name, &file_path_source);

            let output = std::process::Command::new("sh")
                .arg(&file_path_source)
                .output()
                .expect("Error, something went wrong when trying to execute script.");

            io::stdout()
                .write_all(&output.stdout)
                .expect("Couldn't write to stdout");
            io::stderr()
                .write_all(&output.stderr)
                .expect("Couldn't write to stdout");
        }
    }

    Ok(())
}

// fn copy_files(source_dir: &Path, dest_dir: &Path) {
//     use std::process::Command;
//     Command::new("sh")
//         .arg("-c")
//         .arg("cp")
//         .arg("--reflink=auto")
//         .arg("--recursive")
//         .arg(format!("{}", source_dir))
//         .arg(format!("{}", dest_dir))
//         .output()
// }
