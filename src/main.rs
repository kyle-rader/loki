pub mod git;
pub mod pruning;

use clap::Parser;
use git::{
    git_branches, git_command_lines, git_command_status, git_commands_status, git_current_branch,
};
use pruning::is_pruned_branch;
use time::OffsetDateTime;

#[derive(Parser)]
#[clap(version, about, author)]
enum Cli {
    /// Create a new branch from HEAD and push it to origin.
    /// Set a prefix for all new branch names with the env var LOKI_NEW_PREFIX
    #[clap(visible_alias = "n")]
    New {
        /// List of names to join with dashes to form a valid branch name.
        name: Vec<String>,
    },

    /// Push the current branch to origin with --set-upstream
    #[clap(visible_alias = "p")]
    Push {
        /// Use --force-with-lease
        #[clap(short, long)]
        force: bool,
    },

    /// Pull with --prune deleting local branches pruned from the remote.
    Pull,
    /// Fetch with --prune deleting local branches pruned from the remote.
    Fetch,
    /// Add, commit, and push using a timestamp based commit message.
    Save {
        /// Include all untracked (new) files.
        #[clap(short, long)]
        all: bool,
        /// Optional message to include. Each MESSAGE will be joined on whitespace and appended after timestamp.
        message: Vec<String>,
    },
}

const LOKI_NEW_PREFIX: &str = "LOKI_NEW_PREFIX";

fn main() -> Result<(), String> {
    let cli = Cli::parse();

    match &cli {
        Cli::New { name } => new_branch(name),
        Cli::Push { force } => push_branch(*force),
        Cli::Pull => pull_prune(),
        Cli::Fetch => fetch_prune(),
        Cli::Save { all, message } => save(*all, message),
    }
}

fn save(all: bool, message: &Vec<String>) -> Result<(), String> {
    let Ok(now) = OffsetDateTime::now_local() else { return Err(String::from("could not get current time"))};
    let selector_option = match all {
        true => "--all",
        false => "--update",
    };

    git_commands_status(vec![
        ("add files", vec!["add", selector_option]),
        (
            "commit",
            vec![
                "commit",
                "--message",
                format!("lk save [{now}] | {}", message.join(" ")).as_str(),
            ],
        ),
        ("push", vec!["push"]),
    ])?;

    Ok(())
}

fn new_branch(name: &Vec<String>) -> Result<(), String> {
    if name.len() == 0 {
        return Err(String::from("name cannot be empty."));
    }

    let mut name = name.join("-");

    if let Ok(prefix) = std::env::var(LOKI_NEW_PREFIX) {
        eprintln!("Using prefix from env var {LOKI_NEW_PREFIX}={prefix}");
        name = format!("{prefix}{name}");
    }

    git::git_commands_status(vec![
        (
            "create new branch",
            vec!["switch", "--create", name.as_str()],
        ),
        (
            "push to origin",
            vec!["push", "--set-upstream", "origin", name.as_str()],
        ),
    ])?;

    Ok(())
}

fn push_branch(force: bool) -> Result<(), String> {
    let current_branch = git_current_branch()?;

    if current_branch.to_ascii_lowercase() == "head" {
        return Err(String::from(
            "HEAD is currently detached, no branch to push!",
        ));
    }

    let mut args = vec!["push", "--set-upstream"];
    if force {
        args.push("--force-with-lease");
    }
    args.push("origin");
    args.push(current_branch.as_str());
    let args = args;

    git_command_status("push", args)?;

    Ok(())
}

fn pull_prune() -> Result<(), String> {
    prune("pull")
}

fn fetch_prune() -> Result<(), String> {
    prune("fetch")
}

fn prune(cmd: &str) -> Result<(), String> {
    let current_branch = git_current_branch()?;
    let branches = git_branches()?;

    for line in git_command_lines("pull with pruning", vec![cmd, "--prune"])?.into_iter() {
        println!("{line}");
        if let Some(pruned_branch) = is_pruned_branch(line) {
            if pruned_branch.cmp(&current_branch).is_eq() {
                eprintln!(
                    "⚠️ Cannot delete pruned branch {pruned_branch} because HEAD is pointing to it."
                );
            } else if branches.contains(&pruned_branch) {
                if let Err(err) = git_command_status(
                    format!("delete branch {pruned_branch}").as_str(),
                    vec!["branch", "-D", pruned_branch.as_str()],
                ) {
                    eprintln!("Failed to delete pruned branch {pruned_branch}: {err:?}")
                }
            }
        }
    }

    Ok(())
}
