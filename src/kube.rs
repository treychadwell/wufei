use colored::*;
use rand::seq::SliceRandom;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};
use std::thread;

use crate::utils;
use std::collections::HashMap;
use std::str;

#[derive(Debug)]
pub struct LogRecorderConfig {
    namespace: String,
    kube_config: String,
    file: bool,
}

#[derive(Debug, Clone)]
pub struct PodInfo {
    name: String,
    container: String,
}

impl LogRecorderConfig {
    pub fn new(namespace: String, kube_config: String, file: bool) -> LogRecorderConfig {
        LogRecorderConfig {
            namespace: namespace,
            kube_config: kube_config,
            file: file,
        }
    }
}

// Returns a Hashmap of Log FilePath, PodInfo <Returns Hashmap <String Podinfo>
pub fn run_logs(log_options: &LogRecorderConfig) {
    let pod_vec = get_all_pod_info(&log_options.namespace);
    let pod_hashmap = generate_hashmap(pod_vec);
    run_cmd(pod_hashmap, &log_options);
}

fn run_cmd(pod_hashmap: HashMap<String, PodInfo>, log_options: &LogRecorderConfig) {
    let mut children = vec![];
    for (k, v) in pod_hashmap {
        // Do this to avoid lifetimes on LogRecorderConfig
        let namespace = log_options.namespace.clone();
        let kube_config = log_options.kube_config.clone();
        let file = log_options.file.clone();
        children.push(thread::spawn(move || {
            run_individual(
                k.to_string(),
                &v,
                namespace.to_owned(),
                kube_config.to_owned(),
                file.to_owned(),
            )
        }));
    }

    for child in children {
        let _ = child.join();
    }
}

// TODO: Make colored pod name and option to differentiate between logs being streamed
fn run_individual(k: String, v: &PodInfo, namespace: String, kube_config: String, file: bool) {
    let mut kube_cmd = Command::new("kubectl");
    let container = get_app_container(&v.container);
    let out_file = File::create(&k.to_string()).unwrap();
    let deploy_string = "deployment/".to_string() + &container;
    let colors_vec = vec![
        "green",
        "red",
        "yellow",
        "blue",
        "cyan",
        "magenta",
        "white",
        "bright black",
        "bright red",
        "bright green",
        "bright yellow",
        "bright blue",
        "bright magenta",
        "bright cyan",
    ];
    let color = colors_vec.choose(&mut rand::thread_rng()); // get random color
    let str_color = color.unwrap(); // unwrap random

    // build arguments based off LogRecorderConfiguration
    // If kube_config is not empty, use kube config
    if kube_config.len() != 0 {
        kube_cmd.arg("--kubeconfig");
        kube_cmd.arg(&kube_config);
    }

    kube_cmd.arg("logs");
    kube_cmd.arg("-f");
    kube_cmd.arg(&deploy_string);
    kube_cmd.arg(&container);
    kube_cmd.arg("-n");
    kube_cmd.arg(&namespace);

    if file {
        kube_cmd.stdout(Stdio::from(out_file));
    } else {
        kube_cmd.stdout(Stdio::piped());
    }

    // Spin off child process
    let output = kube_cmd
        .spawn()
        .unwrap()
        .stdout
        .ok_or_else(|| "Unable to follow kube logs")
        .unwrap();

    // TODO: add loop here to write to a file forever until a siglkill happens to stream to a file
    let reader = BufReader::new(output);
    let mut log_prefix = "[pod] ".to_string();
    log_prefix = log_prefix + "[" + &v.name + "]";

    log_prefix = log_prefix.color(str_color.to_string()).to_string();
    reader
        .lines()
        .filter_map(|line| line.ok())
        .for_each(|line| println!("{}: {}", &log_prefix, line));
}

fn get_app_container(containers: &str) -> String {
    // Need to split the whitespaces, and get the first container name only (not worried about
    // istio resources right now)
    let container = containers.split_whitespace();
    let container_vec: Vec<&str> = container.collect();
    return container_vec[0].to_string();
}

fn get_all_pod_info(namespace: &str) -> Vec<String> {
    let output = Command::new("kubectl")
        .args(&["--kubeconfig", "kube.config"])
        .args(&["get", "pods"])
        .args(&["-n", &namespace])
        .args(&["-o", "jsonpath={range .items[*]}{.metadata.name} {.spec['containers', 'initContainers'][*].name}\n{end}"])
        .output()
        .expect("Failed to get kubernetes pods");

    // if error handle it here
    // if output.stderr handle
    let pods = str::from_utf8(&output.stdout).unwrap();
    let pods_vec: Vec<&str> = pods.split("\n").collect();
    utils::str_to_string(pods_vec)
}

fn generate_hashmap(pod_vec: Vec<String>) -> HashMap<String, PodInfo> {
    let mut pods_hashmap = HashMap::new();
    for info in pod_vec {
        // Empty namespaces happen for some reason.  Breaks the indices
        if info == "" {
            continue;
        }

        // explode on whitespace to seperate concerns
        let pod_info = info.split_whitespace();
        let mut pod_info_vec: Vec<&str> = pod_info.collect();
        let single_pod_vec = pod_info_vec.split_off(0);

        // fix the name split and container split
        let string_vec = utils::str_to_string(single_pod_vec);
        let file_path = "/tmp/".to_owned() + &string_vec[0] + ".txt";

        let name = &string_vec[0];
        let containers = &string_vec[1];

        pods_hashmap.insert(
            file_path,
            PodInfo {
                name: name.to_string(),
                container: containers.to_string(),
            },
        );
    }

    pods_hashmap
}
