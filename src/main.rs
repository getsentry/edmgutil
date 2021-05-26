use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

use anyhow::{bail, Error};
use argh::FromArgs;
use dialoguer::Password;
use uuid::Uuid;
use which::which;

/// A utility to mount an encrypted zip into a new encrypted dmg.
#[derive(Debug, FromArgs)]
struct Cli {
    /// provide a password instead of prompting
    #[argh(option, short = 'p', long = "password")]
    password: Option<String>,
    /// the extra size for the encrypted DMG in megabytes
    #[argh(option, short = 's', long = "extra-size", default = "100")]
    extra_size: usize,
    /// the volume name of the dmg
    #[argh(option, short = 'n', long = "name")]
    volume_name: Option<String>,
    /// keep the dmg?
    #[argh(switch, short = 'k', long = "keep-dmg")]
    keep_dmg: bool,
    /// the path of the input zip archive
    #[argh(positional)]
    path: PathBuf,
}

fn make_dmg(path: &Path, volume_name: &str, size: usize, password: &str) -> Result<(), Error> {
    let mut child = Command::new("hdiutil")
        .arg("create")
        .arg("-megabytes")
        .arg(size.to_string())
        .arg("-ov")
        .arg("-volname")
        .arg(volume_name)
        .arg("-fs")
        .arg("HFS+")
        .arg("-encryption")
        .arg("AES-256")
        .arg("-stdinpass")
        .arg(&path)
        .stdin(Stdio::piped())
        .spawn()?;
    let mut stdin = child.stdin.take().unwrap();
    stdin.write_all(password.as_bytes())?;
    drop(stdin);
    child.wait()?;
    Ok(())
}

fn mount_dmg(path: &Path, password: &str) -> Result<PathBuf, Error> {
    let mut child = Command::new("hdiutil")
        .arg("attach")
        .arg("-stdinpass")
        .arg(path)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()?;
    let mut stdin = child.stdin.take().unwrap();
    stdin.write_all(password.as_bytes())?;
    drop(stdin);
    let output = child.wait_with_output()?;
    let to_parse = std::str::from_utf8(&output.stdout)?;
    for line in to_parse.lines() {
        if !line.contains("\tApple_HFS") {
            continue;
        }
        return Ok(PathBuf::from(
            line.trim_end_matches(&['\n'][..])
                .splitn(3, '\t')
                .nth(2)
                .unwrap(),
        ));
    }

    bail!("failed to mount dmg");
}

fn get_uncompressed_zip_size(path: &Path) -> Result<usize, Error> {
    let child = Command::new("7z")
        .arg("l")
        .arg(path)
        .stdout(Stdio::piped())
        .spawn()?
        .wait_with_output()?;
    let output = std::str::from_utf8(&child.stdout)?;
    let last_line = output.trim().lines().last().unwrap();
    let bytes: u64 = last_line.split_ascii_whitespace().nth(2).unwrap().parse()?;
    Ok((bytes / 1024) as usize + 1)
}

fn check_password(path: &Path, password: &str) -> Result<bool, Error> {
    let child = Command::new("7z")
        .arg("t")
        .arg(&format!("-p{}", password))
        .arg(path)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?
        .wait_with_output()?;
    let output = std::str::from_utf8(&child.stdout)?;
    let err = std::str::from_utf8(&child.stderr)?;
    Ok(!err.contains("ERROR: Wrong password") && output.contains("Everything is Ok"))
}

fn extract(src: &Path, dst: &Path, password: &str) -> Result<(), Error> {
    Command::new("7z")
        .arg("x")
        .arg("-bsp2")
        .arg(&format!("-p{}", password))
        .arg("-y")
        .arg(src)
        .current_dir(dst)
        .stdout(Stdio::null())
        .spawn()?
        .wait()?;
    Ok(())
}

fn secure_volume(path: &Path) -> Result<(), Error> {
    fs::write(path.join(".metadata_never_index"), "")?;
    Command::new("mdutil")
        .arg("-E")
        .arg("-i")
        .arg("off")
        .arg(path)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?
        .wait()?;
    Ok(())
}

fn main() -> Result<(), Error> {
    let mut cli: Cli = argh::from_env();

    if !which("7z").is_ok() {
        bail!("7z is not available");
    }
    if !which("hdiutil").is_ok() {
        bail!("hdiutil is not available");
    }

    let password = match cli.password.take() {
        Some(password) => password,
        None => Password::new().with_prompt("password").interact()?,
    };

    if !fs::metadata(&cli.path).map_or(false, |x| x.is_file()) {
        bail!("source archive does not exist or is not a file");
    }
    if !check_password(&cli.path, &password)? {
        bail!("invalid password");
    }

    let volume_name = match cli.volume_name {
        Some(ref name) => name.as_str(),
        None => cli
            .path
            .as_path()
            .file_stem()
            .and_then(|x| x.to_str())
            .unwrap_or("Data"),
    };

    let size = get_uncompressed_zip_size(&cli.path)? + cli.extra_size;

    println!("[1] Creating encrypted DMG");
    let path =
        std::env::temp_dir().join(format!("encrypted-{}-{}.dmg", Uuid::new_v4(), volume_name));
    make_dmg(&path, volume_name, size, &password)?;
    println!("[2] Mounting DMG");
    let mounted_at = mount_dmg(&path, &password)?;
    println!("[3] Securing mounted volume");
    secure_volume(&mounted_at)?;
    println!("[4] Extracting encrypted zip");
    extract(&cli.path, &mounted_at, &password)?;
    if cli.keep_dmg {
        println!("Placed encrypted DMG at: {}", path.display());
    } else {
        fs::remove_file(&path)?;
    }
    println!("Mounted encrypted DMG at: {}", mounted_at.display());
    println!("Ummount with: umount \"{}\"", mounted_at.display());
    Ok(())
}
