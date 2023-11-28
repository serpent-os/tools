// SPDX-FileCopyrightText: Copyright © 2020-2023 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0
use std::env::set_current_dir;
use std::fs::{copy, create_dir_all, remove_dir, write};
use std::path::{Path, PathBuf};

use nix::libc::SIGCHLD;
use nix::mount::{mount, umount2, MntFlags, MsFlags};
use nix::sched::{clone, CloneFlags};
use nix::sys::wait::waitpid;
use nix::unistd::{close, getgid, getuid, pipe, pivot_root, read, sethostname, Uid};

pub type Error = Box<dyn std::error::Error + Send + Sync + 'static>;

pub struct Container {
    root: PathBuf,
    work_dir: Option<PathBuf>,
    // TODO: Strongly typed & ro variant
    binds: Vec<(PathBuf, PathBuf)>,
    networking: bool,
    hostname: Option<String>,
}

impl Container {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self {
            root: root.into(),
            work_dir: None,
            binds: vec![],
            networking: false,
            hostname: None,
        }
    }

    pub fn work_dir(self, work_dir: impl Into<PathBuf>) -> Self {
        Self {
            work_dir: Some(work_dir.into()),
            ..self
        }
    }

    pub fn bind(mut self, host: impl Into<PathBuf>, guest: impl Into<PathBuf>) -> Self {
        self.binds.push((host.into(), guest.into()));
        self
    }

    pub fn networking(self, enabled: bool) -> Self {
        Self {
            networking: enabled,
            ..self
        }
    }

    pub fn hostname(self, hostname: impl ToString) -> Self {
        Self {
            hostname: Some(hostname.to_string()),
            ..self
        }
    }

    pub fn run(self, mut f: impl FnMut() -> Result<(), Error>) -> Result<(), Error> {
        static mut STACK: [u8; 4 * 1024 * 1024] = [0u8; 4 * 1024 * 1024];

        let rootless = !Uid::effective().is_root();

        // Pipe to synchronize parent & child
        let sync = pipe()?;

        let mut flags = CloneFlags::CLONE_NEWNS
            | CloneFlags::CLONE_NEWPID
            | CloneFlags::CLONE_NEWIPC
            | CloneFlags::CLONE_NEWUTS;

        if rootless {
            flags |= CloneFlags::CLONE_NEWUSER;
        }

        if !self.networking {
            flags |= CloneFlags::CLONE_NEWNET;
        }

        let pid = unsafe {
            clone(
                Box::new(|| match enter(&self, sync, &mut f) {
                    Ok(_) => 0,
                    Err(e) => {
                        eprintln!("Error: {e}");
                        1
                    }
                }),
                &mut STACK,
                flags,
                Some(SIGCHLD),
            )?
        };

        if rootless {
            // Update uid / gid map to map current user to root in container
            write(format!("/proc/{pid}/setgroups"), "deny")?;
            write(format!("/proc/{pid}/uid_map"), format!("0 {} 1", getuid()))?;
            write(format!("/proc/{pid}/gid_map"), format!("0 {} 1", getgid()))?;
        }

        // Allow child to continue
        close(sync.1)?;

        waitpid(pid, None)?;

        Ok(())
    }
}

fn enter(
    container: &Container,
    sync: (i32, i32),
    mut f: impl FnMut() -> Result<(), Error>,
) -> Result<(), Error> {
    // Close unused write end
    close(sync.1)?;
    // Got EOF, continue
    read(sync.0, &mut [0u8; 1])?;
    close(sync.0)?;

    setup(container)?;

    f()
}

fn setup(container: &Container) -> Result<(), Error> {
    if container.networking {
        setup_networking(&container.root)?;
    }

    pivot(&container.root, &container.binds)?;

    setup_root_user()?;

    if let Some(hostname) = &container.hostname {
        sethostname(hostname)?;
    }

    if let Some(dir) = &container.work_dir {
        set_current_dir(dir)?;
    }

    Ok(())
}

fn pivot(root: &Path, binds: &[(PathBuf, PathBuf)]) -> Result<(), Error> {
    const OLD_PATH: &str = "old_root";

    let old_root = root.join(OLD_PATH);

    add_mount(None, "/", None, MsFlags::MS_REC | MsFlags::MS_PRIVATE)?;
    add_mount(Some(root), root, None, MsFlags::MS_BIND)?;

    for (host, guest) in binds {
        let source = host.canonicalize()?;
        let target = root.join(guest.strip_prefix("/").unwrap_or(guest));
        add_mount(Some(source), target, None, MsFlags::MS_BIND)?;
    }

    enusure_directory(&old_root)?;
    pivot_root(root, &old_root)?;

    set_current_dir("/")?;

    add_mount(Some("proc"), "proc", Some("proc"), MsFlags::empty())?;
    add_mount(Some("tmpfs"), "tmp", Some("tmpfs"), MsFlags::empty())?;
    add_mount(
        Some(format!("/{OLD_PATH}/sys").as_str()),
        "sys",
        None,
        MsFlags::MS_BIND | MsFlags::MS_REC | MsFlags::MS_SLAVE,
    )?;
    add_mount(
        Some(format!("/{OLD_PATH}/dev").as_str()),
        "dev",
        None,
        MsFlags::MS_BIND | MsFlags::MS_REC | MsFlags::MS_SLAVE,
    )?;

    umount2(OLD_PATH, MntFlags::MNT_DETACH)?;
    remove_dir(OLD_PATH)?;

    Ok(())
}

fn setup_root_user() -> Result<(), Error> {
    enusure_directory("/etc")?;
    write("/etc/passwd", "root:x:0:0:root::/bin/bash")?;
    write("/etc/group", "root:x:0:")?;
    Ok(())
}

fn setup_networking(root: &Path) -> Result<(), Error> {
    enusure_directory(root.join("etc"))?;
    copy("/etc/resolv.conf", root.join("etc/resolv.conf"))?;
    copy("/etc/protocols", root.join("etc/protocols"))?;
    Ok(())
}

fn enusure_directory(path: impl AsRef<Path>) -> Result<(), Error> {
    let path = path.as_ref();
    if !path.exists() {
        create_dir_all(path)?;
    }
    Ok(())
}

fn add_mount<T: AsRef<Path>>(
    source: Option<T>,
    target: T,
    fs_type: Option<&str>,
    flags: MsFlags,
) -> Result<(), Error> {
    enusure_directory(&target)?;
    mount(
        source.as_ref().map(AsRef::as_ref),
        target.as_ref(),
        fs_type,
        flags,
        Option::<&str>::None,
    )?;
    Ok(())
}
