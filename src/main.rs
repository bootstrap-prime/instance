#![feature(let_chains)]

use clap::{App, Arg};
use miette::{bail, Context, IntoDiagnostic};
use serde::Deserialize;
use std::collections::HashMap;
use std::ffi::OsString;
use std::io::{self, Write};
use std::{env, path::PathBuf};
use std::{fs, fs::OpenOptions};

const DEFAULT_CONFIG_NAME: &str = "instance_config.toml";
const SETTINGS_DEFAULT_BEHAVIOR: Behavior = Behavior::Fail;
const USE_NIX_SHELL: bool = true;

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

fn main() -> miette::Result<()> {
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

    let current_dir = env::current_dir().into_diagnostic()?;

    // figures out where the configuration is
    // order of priority is (most important to least) [given config -> env var -> default directory (which is ~/.templates)]
    let template_dir_path = matches
        .value_of("config")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            env::var("INSTANCE_TEMPLATE_DIR")
                .map(PathBuf::from)
                .unwrap_or_else(|_| {
                    home::home_dir()
                        .map(|mut path| {
                            path.push(".templates");
                            path
                        })
                        .expect("Couldn't find home directory")
                })
        });

    // read data from config file
    let file_input: Config = toml::from_str(&{
        let config_path: PathBuf = [&template_dir_path, &PathBuf::from(DEFAULT_CONFIG_NAME)]
            .iter()
            .collect();
        fs::read_to_string(&config_path).into_diagnostic()?
    })
    .into_diagnostic()
    .context("Failed to parse configuration")?;

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
    let template_data = validate_template(&template_dir_path, &settings_data, &template_data)?;
    // validate projects: ensure all projects are valid
    let project_data: Option<Vec<ValidProject>> = project_data
        .as_ref()
        .map(|projects| validate_project(&template_data, &projects))
        .transpose()?;

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
        let possible_project = project_data
            .as_ref()
            .map(|project| {
                project
                    .iter()
                    .find(|element| element.call_name.eq_ignore_ascii_case(o))
            })
            .flatten();

        if let Some(project) = possible_project {
            instantiate_project(
                project,
                &current_dir,
                &template_dir_path,
                &settings_data,
                &template_data,
            )?;
        } else {
            bail!("{} is not a project on record.", o);
        }
    }

    // TODO: copy template to SPECIFIED directory instead of WORKING directory.
    // copy template to working directory
    if let Some(o) = matches.value_of("template") {
        let get_val = template_data
            .iter()
            .find(|&element| element.call_name.eq_ignore_ascii_case(o));

        match get_val {
            None => bail!("{} is not a template on record.", o),
            Some(element) => {
                instantiate_template(element, &current_dir, &template_dir_path, &settings_data)?;
            }
        };
    }

    Ok(())
}

pub struct ValidProject<'a> {
    templates: Vec<&'a ValidTemplate>,
    call_name: String,
}

pub struct ValidTemplate {
    path: PathBuf,
    ttype: TemplateType,
    call_name: String,
    rename: OsString,
    behavior: Behavior,
}

// check to see if all templates exist and are valid
fn validate_template<'a>(
    root_data: &'a PathBuf,
    settings: &Settings,
    template_data: &'a [Template],
) -> miette::Result<Vec<ValidTemplate>> {
    let (invalid_templates, valid_templates): (Vec<&Template>, Vec<&Template>) = template_data
        .iter()
        .partition(|element| !root_data.as_path().join(element.path.as_str()).exists());

    if !invalid_templates.is_empty() {
        bail!("Path referenced by template could not be found: \n{}", {
            &invalid_templates
                .iter()
                .map(|element| element.call_name.to_string())
                .collect::<Vec<String>>()
                .join("\n")
        });
    }

    let validated_templates = valid_templates
        .iter()
        .map(|template| {
            let possiblepath = PathBuf::from(&template.path)
                .canonicalize()
                .unwrap_or_else(|_| root_data.as_path().join(&template.path));

            ValidTemplate {
                path: possiblepath,
                ttype: template.ttype,
                call_name: template.call_name,
                rename: template
                    .rename
                    .map(|e| e.into())
                    .unwrap_or_else(|| possiblepath.file_name().unwrap().into()),
                behavior: template.behavior.unwrap_or_else(|| {
                    settings
                        .default_behavior
                        .unwrap_or(SETTINGS_DEFAULT_BEHAVIOR)
                }),
            }
        })
        .collect();

    Ok(validated_templates)
}

// check to see if all subtemplates exists and if the project's defined properly
fn validate_project<'a>(
    template_data: &'a [ValidTemplate],
    project_data: &'a [Project],
) -> miette::Result<Vec<ValidProject<'a>>> {
    project_data
        .iter()
        .map(|project| {
            // Vec<String> -> Vec<&ValidTemplate>
            // let templates =
            project
                .templates
                .iter()
                .map(|template_name| {
                    template_data
                        .iter()
                        .find(|actual_template| actual_template.call_name == *template_name)
                        .ok_or(miette::miette!(
                            "Project {} references nonexistent template: \n {}",
                            project.call_name,
                            template_name
                        ))
                })
                .collect::<Result<Vec<&ValidTemplate>, _>>()
                // let templates = templates.
                .map(|templates| ValidProject {
                    templates,
                    call_name: project.call_name,
                })

            // templates
        })
        .collect()
}

// instance a project (and all the templates in it)
fn instantiate_project(
    element: &Project,
    file_path_destin: &PathBuf,
    template_source_path: &PathBuf,
    settings_data: &Settings,
    templates: &[Template],
) -> miette::Result<()> {
    use rayon::prelude::*;

    element.templates.par_iter().try_for_each(|template| {
        let template_ref = &templates
            .iter()
            .find(|&element| element.call_name.eq_ignore_ascii_case(&template))
            // we are assuming the validation logic caught this
            .unwrap();

        instantiate_template(
            template_ref,
            &file_path_destin,
            &template_source_path,
            &settings_data,
        )
    })?;

    Ok(())
}

// instance a template (whether script or file)
fn instantiate_template(
    element: &Template,
    base_path: &PathBuf,
    template_source_path: &PathBuf,
    settings_data: &Settings,
) -> miette::Result<()> {
    let file_path_source: PathBuf = [&template_source_path, &PathBuf::from(element.path.as_str())]
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
                        bail!("file already exists and the setting for this template on conflict is failure.")
                    }
                    Behavior::Append => {
                        let source_data =
                            fs::read_to_string(&file_path_source).into_diagnostic()?;
                        let mut dest_file = OpenOptions::new()
                            .write(true)
                            .append(true)
                            .open(&file_path_destin)
                            .into_diagnostic()?;
                        dest_file
                            .write_all(source_data.as_bytes())
                            .into_diagnostic()?;
                    }
                    Behavior::Overwrite => {
                        fs::remove_file(&file_path_destin).into_diagnostic()?;
                        fs::copy(&file_path_source, &file_path_destin).into_diagnostic()?;
                    }
                }
            } else {
                fs::copy(&file_path_source, &file_path_destin).into_diagnostic()?;
            }
        }
        TemplateType::Script => {
            // for things like cargo init and git init
            // automatically integrates with nix-shell shebangs for user convenience.
            println!("script: {} at {:?}", &element.call_name, &file_path_source);

            let output = std::process::Command::new(if USE_NIX_SHELL { "nix-shell" } else { "sh" })
                .arg(&file_path_source)
                .output()
                .into_diagnostic()
                .context(format!(
                    "something went wrong when trying to execute {}",
                    &file_path_source.display(),
                ))?;

            io::stdout()
                .write_all(&output.stdout)
                .into_diagnostic()
                .context("Couldn't write to stdout")?;
            io::stderr()
                .write_all(&output.stderr)
                .into_diagnostic()
                .context("Couldn't write to stderr")?;
        }
    }

    Ok(())
}
