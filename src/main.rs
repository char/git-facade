use git2::{Repository, Commit, Time};

use std::fs::File;
use std::fs;

use std::path::Path;

use std::io::prelude::*;
use std::process::exit;

use toml::Value;

fn get_commits_from_repo(repo: &Repository) -> Vec<Commit> {
    let mut revwalk = repo.revwalk().unwrap();
    revwalk.push_head().unwrap();

    revwalk.filter_map(|oid| {
        let commit = repo.find_commit(oid.unwrap()).unwrap();
        if commit.parents().len() > 1 { return None; }

        Some(commit)
    }).collect()
}

fn get_latest_date_from_repo(repo: &Repository) -> Option<Time> {
    let workdir = repo.workdir().unwrap();
    let path = workdir.join(&Path::new("latest-update.txt"));

    if let Ok(time) = fs::read_to_string(path) {
        let split: Vec<&str> = time.split_whitespace().collect();

        let time64 = split[0].parse::<i64>().unwrap();
        let offset32 = split[1].parse::<i32>().unwrap();

        return Some(Time::new(time64, offset32));
    }

    None
}

fn write_latest_date_to_repo(repo: &Repository, latest_date: &Time) {
    let workdir = repo.workdir().unwrap();
    let path = workdir.join(&Path::new("latest-update.txt"));

    fs::write(path, format!("{} {}", latest_date.seconds(), latest_date.offset_minutes())).unwrap();
}

fn add_fake_commit_to_repo(repo: &Repository, message: &str, orig_commit: &Commit) {
    let signature = orig_commit.author();

    let mut index = repo.index().unwrap();

    let rel_file_path = Path::new("scratch-file.bin");
    let file_path = repo.workdir().unwrap().join(&rel_file_path);

    fs::write(file_path, orig_commit.id().as_bytes()).unwrap();

    index.add_path(&rel_file_path).unwrap();
    index.write().unwrap();

    let tree_oid = index.write_tree().unwrap();
    let tree = repo.find_tree(tree_oid).unwrap();

    match repo.refname_to_id("HEAD") {
        Ok(head_oid) => {
            let parent_commits = [&repo.find_commit(head_oid).unwrap()];
            repo.commit(Some("HEAD"), &signature, &signature, message, &tree, &parent_commits).unwrap();
        },

        Err(_e) => {
            let parent_commits = vec![];
            repo.commit(Some("HEAD"), &signature, &signature, message, &tree, &parent_commits).unwrap();
        }
    };
}

fn main() {
    let mut config_file_contents = String::new();

    {
        let mut config_file = match File::open(".gitfacade.toml") {
            Ok(config_file) => config_file,
            Err(_e) => {
                eprintln!("Failed to read file: .gitfacade.toml");
                exit(1);
            }
        };

        config_file.read_to_string(&mut config_file_contents).unwrap();
    }

    let config = config_file_contents.parse::<Value>().unwrap();

    let mut latest_date = Time::new(0, 0);

    let repo_dir = config["repo"].as_str().unwrap();

    let repo = match Repository::open(repo_dir) {
        Ok(repo) => {
            if let Some(repo_latest_date) = get_latest_date_from_repo(&repo) {
                latest_date = repo_latest_date
            }

            repo
        },
        Err(_e) => {
            match Repository::init(repo_dir) {
                Ok(repo) => {
                    repo
                },
                Err(_e) => {
                    eprintln!("Failed to create or open repository.");
                    exit(1);
                }
            }
        }
    };

    let start_date = latest_date;
    let target_repos = config["repos"].as_table().unwrap();

    for (name, target_repo_dir_value) in target_repos {
        let target_repo_dir = target_repo_dir_value.as_str().unwrap();
        let target_repo = Repository::open(target_repo_dir).unwrap();

        let commits = get_commits_from_repo(&target_repo);
        for commit in commits {
            let commit_time = commit.time();

            if commit_time >= start_date {
                if commit_time > latest_date {
                    latest_date = commit_time;
                    write_latest_date_to_repo(&repo, &latest_date);
                }

                add_fake_commit_to_repo(&repo, format!("Façade commit: {}", name).as_str(), &commit);
            }
        }
    }

    println!("{}", repo.workdir().unwrap().to_str().unwrap());
}
