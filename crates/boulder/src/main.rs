use std::{path::PathBuf, process::Command};

use clap::Parser;
use tokio::fs::{create_dir, remove_dir_all};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    if args.chroot.exists() {
        remove_dir_all(&args.chroot).await?;
    }
    create_dir(&args.chroot).await?;

    let mut client = moss::Client::new(&args.root)
        .await?
        .ephemeral(&args.chroot)?;

    client.install(BASE_PACKAGES, true).await?;

    container::run(args.chroot, move || {
        let mut child = Command::new("/bin/bash")
            .arg("--login")
            .env_clear()
            .env("HOME", "/root")
            .env("PATH", "/usr/bin:/usr/sbin")
            .env("TERM", "xterm-256color")
            .spawn()?;

        child.wait()?;

        Ok(())
    })?;

    Ok(())
}

#[derive(Debug, Parser)]
#[command()]
struct Args {
    #[arg(short = 'D', long = "directory", global = true, default_value = "/")]
    root: PathBuf,
    chroot: PathBuf,
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
