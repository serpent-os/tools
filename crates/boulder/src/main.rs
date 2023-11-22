use std::{
    path::{Path, PathBuf},
    process::Command,
};

use clap::Parser;
use color_eyre::eyre::{eyre, Result};
use tokio::fs::{create_dir_all, remove_dir_all};

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    color_eyre::install()?;

    let is_root = is_root();

    let _config = if let Some(dir) = args.config_dir {
        config::Manager::custom(dir)
    } else if is_root {
        config::Manager::system("/", "boulder")
    } else {
        config::Manager::user("boulder")?
    };

    let cache = cache_dir(is_root, args.cache_dir)?;

    let ephemeral_root = cache.join("test-root");
    recreate_dir(&ephemeral_root).await?;

    let mut client = moss::Client::new(&args.moss_root)
        .await?
        .ephemeral(&ephemeral_root)?;

    client.install(BASE_PACKAGES, true).await?;

    container::run(&ephemeral_root, move || {
        let mut child = Command::new("/bin/bash")
            .arg("--login")
            .env_clear()
            .env("HOME", "/root")
            .env("PATH", "/usr/bin:/usr/sbin")
            .env("TERM", "xterm-256color")
            .spawn()?;

        child.wait()?;

        Ok(())
    })
    .map_err(|e| eyre!("container error: {e}"))?;

    Ok(())
}

#[derive(Debug, Parser)]
#[command()]
struct Args {
    #[arg(long, default_value = "/")]
    moss_root: PathBuf,
    #[arg(long)]
    config_dir: Option<PathBuf>,
    #[arg(long)]
    cache_dir: Option<PathBuf>,
}

const BASE_PACKAGES: &[&str] = &[
    "bash",
    "boulder",
    "coreutils",
    "dash",
    "dbus",
    "dbus-broker",
    "file",
    "gawk",
    "git",
    "grep",
    "gzip",
    "inetutils",
    "iproute2",
    "less",
    "linux-kvm",
    "moss",
    "moss-container",
    "nano",
    "neofetch",
    "nss",
    "openssh",
    "procps",
    "python",
    "screen",
    "sed",
    "shadow",
    "sudo",
    "systemd",
    "unzip",
    "util-linux",
    "vim",
    "wget",
    "which",
];

fn is_root() -> bool {
    use nix::unistd::Uid;

    Uid::effective().is_root()
}

fn cache_dir(is_root: bool, custom: Option<PathBuf>) -> Result<PathBuf> {
    if let Some(dir) = custom {
        Ok(dir)
    } else if is_root {
        Ok(PathBuf::from("/var/cache/boulder"))
    } else {
        Ok(dirs::cache_dir()
            .ok_or_else(|| eyre!("cannot find cache dir, $XDG_CACHE_HOME or $HOME env not set"))?
            .join("boulder"))
    }
}

async fn recreate_dir(path: &Path) -> Result<()> {
    if path.exists() {
        remove_dir_all(&path).await?;
    }
    create_dir_all(&path).await?;
    Ok(())
}
