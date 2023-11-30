use std::env::set_current_dir;
use std::fs::{copy, create_dir, remove_dir, write};
use std::path::Path;

use nix::libc::SIGCHLD;
use nix::mount::{mount, umount2, MntFlags, MsFlags};
use nix::sched::{clone, CloneFlags};
use nix::sys::wait::waitpid;
use nix::unistd::{close, getgid, getuid, pipe, pivot_root, read, sethostname};

type Error = Box<dyn std::error::Error>;

pub fn run(root: impl AsRef<Path>, mut f: impl FnMut() -> Result<(), Error>) -> Result<(), Error> {
    static mut STACK: [u8; 4 * 1024 * 1024] = [0u8; 4 * 1024 * 1024];

    let root = root.as_ref();

    // Pipe to synchronize parent & child
    let sync = pipe()?;

    let pid = unsafe {
        clone(
            Box::new(|| match enter(root, sync, &mut f) {
                Ok(_) => 0,
                Err(e) => {
                    eprintln!("Error: {e}");
                    1
                }
            }),
            &mut STACK,
            CloneFlags::CLONE_NEWNS
                | CloneFlags::CLONE_NEWPID
                | CloneFlags::CLONE_NEWIPC
                | CloneFlags::CLONE_NEWUTS
                | CloneFlags::CLONE_NEWUSER,
            Some(SIGCHLD),
        )?
    };

    // Update uid / gid map to map current user to root in container
    write(format!("/proc/{pid}/setgroups"), "deny")?;
    write(format!("/proc/{pid}/uid_map"), format!("0 {} 1", getuid()))?;
    write(format!("/proc/{pid}/gid_map"), format!("0 {} 1", getgid()))?;

    // Allow child to continue
    close(sync.1)?;

    waitpid(pid, None)?;

    Ok(())
}

fn enter(
    root: &Path,
    sync: (i32, i32),
    mut f: impl FnMut() -> Result<(), Error>,
) -> Result<(), Error> {
    // Close unused write end
    close(sync.1)?;
    // Got EOF, continue
    read(sync.0, &mut [0u8; 1])?;
    close(sync.0)?;

    setup(root)?;

    f()
}

fn setup(root: &Path) -> Result<(), Error> {
    // TODO: conditional networking
    setup_networking(root)?;

    pivot(root)?;

    setup_root_user()?;
    sethostname("boulder")?;

    Ok(())
}

fn pivot(root: &Path) -> Result<(), Error> {
    const OLD_PATH: &str = "old_root";

    let old_root = root.join(OLD_PATH);

    add_mount(None, "/", None, MsFlags::MS_REC | MsFlags::MS_PRIVATE)?;
    add_mount(Some(root), root, None, MsFlags::MS_BIND)?;

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
    write("/etc/passwd", "root:x:0:0:root:/root:/bin/bash")?;
    write("/etc/group", "root:x:0:")?;
    enusure_directory("/root")?;
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
        create_dir(path)?;
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
