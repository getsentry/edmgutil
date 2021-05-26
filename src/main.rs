use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::{Duration, SystemTime},
};

use anyhow::{bail, Error};
use argh::FromArgs;
use dialoguer::Password;
use serde::Deserialize;
use uuid::Uuid;
use which::which;

#[derive(Deserialize, Debug)]
#[serde(rename_all = "kebab-case")]
struct HdiUtilSystemEntity {
    mount_point: Option<PathBuf>,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "kebab-case")]
struct HdiUtilImage {
    system_entities: Vec<HdiUtilSystemEntity>,
}

#[derive(Deserialize, Debug)]
struct HdiUtilInfo {
    images: Vec<HdiUtilImage>,
}

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
    /// the amount of days the image is good to keep (defaults to 7 days)
    #[argh(option, long = "days", default = "7")]
    days: u32,
    /// the path of the input zip archive
    #[argh(positional)]
    path: Option<PathBuf>,
    /// unmounts all expired volumes
    #[argh(switch, long = "unmount-expired")]
    umount_expired: bool,
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

fn secure_volume(path: &Path, days: u32) -> Result<(), Error> {
    let good_until = (SystemTime::now() + Duration::from_secs((days as u64) * 60 * 60 * 24))
        .duration_since(SystemTime::UNIX_EPOCH)?;
    fs::write(path.join(".metadata_never_index"), "")?;
    fs::write(
        path.join(".encrypted-volume-good-until"),
        good_until.as_secs().to_string(),
    )?;
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

fn unmount(path: &Path) -> Result<(), Error> {
    Command::new("umount").arg(path).spawn()?.wait()?;
    Ok(())
}

fn unmount_expired() -> Result<(), Error> {
    let output = Command::new("hdiutil")
        .arg("info")
        .arg("-plist")
        .stdout(Stdio::piped())
        .spawn()?
        .wait_with_output()?;
    let info: HdiUtilInfo = plist::from_bytes(&output.stdout)?;
    let mut encrypted_volumes = vec![];

    for image in &info.images {
        for entity in &image.system_entities {
            if let Some(ref mount_point) = entity.mount_point {
                if let Some(ts) =
                    fs::read_to_string(mount_point.join(".encrypted-volume-good-until"))
                        .ok()
                        .and_then(|x| x.parse().ok())
                        .map(|x| SystemTime::UNIX_EPOCH + Duration::from_secs(x))
                {
                    encrypted_volumes.push((mount_point, ts));
                }
            }
        }
    }

    let now = SystemTime::now();
    for (mount_point, expires) in encrypted_volumes {
        if expires < now {
            println!("Unmounting expired volume {}", mount_point.display());
            unmount(mount_point)?;
        } else {
            println!("Keeping non expired volume {}", mount_point.display());
        }
    }

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

    if cli.umount_expired {
        return unmount_expired();
    }

    let password = match cli.password.take() {
        Some(password) => password,
        None => Password::new().with_prompt("password").interact()?,
    };

    let input_path = match cli.path {
        Some(ref path) => fs::canonicalize(path)?,
        None => bail!("source archive path is required"),
    };

    if !fs::metadata(&input_path).map_or(false, |x| x.is_file()) {
        bail!("source archive is not a file");
    }
    if !check_password(&input_path, &password)? {
        bail!("invalid password");
    }

    let volume_name = match cli.volume_name {
        Some(ref name) => name.as_str(),
        None => input_path
            .file_stem()
            .and_then(|x| x.to_str())
            .unwrap_or("Data"),
    };

    let size = get_uncompressed_zip_size(&input_path)? + cli.extra_size;

    println!("[1] Creating encrypted DMG");
    let path =
        std::env::temp_dir().join(format!("encrypted-{}-{}.dmg", Uuid::new_v4(), volume_name));
    make_dmg(&path, volume_name, size, &password)?;
    println!("[2] Mounting DMG");
    let mounted_at = mount_dmg(&path, &password)?;
    println!("[3] Securing mounted volume");
    secure_volume(&mounted_at, cli.days)?;
    println!("[4] Extracting encrypted zip");
    extract(&input_path, &mounted_at, &password)?;
    if cli.keep_dmg {
        println!("Placed encrypted DMG at: {}", path.display());
    } else {
        fs::remove_file(&path)?;
    }
    println!("Mounted encrypted DMG at: {}", mounted_at.display());
    println!("Ummount with: umount \"{}\"", mounted_at.display());
    Ok(())
}
