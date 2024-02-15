use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::{Duration, SystemTime},
};

use anyhow::{bail, Error};
use serde::Deserialize;

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

pub fn make_dmg(path: &Path, volume_name: &str, size: usize, password: &str) -> Result<(), Error> {
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
        .arg(path)
        .stdin(Stdio::piped())
        .spawn()?;
    let mut stdin = child.stdin.take().unwrap();
    stdin.write_all(password.as_bytes())?;
    drop(stdin);
    child.wait()?;
    Ok(())
}

pub fn mount_dmg(path: &Path, password: &str) -> Result<PathBuf, Error> {
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

pub fn list_volumes() -> Result<Vec<(PathBuf, SystemTime)>, Error> {
    let output = Command::new("hdiutil")
        .arg("info")
        .arg("-plist")
        .stdout(Stdio::piped())
        .spawn()?
        .wait_with_output()?;
    let info: HdiUtilInfo = plist::from_bytes(&output.stdout)?;
    let mut encrypted_volumes = vec![];

    for image in info.images {
        for entity in image.system_entities {
            if let Some(mount_point) = entity.mount_point {
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

    Ok(encrypted_volumes)
}

pub fn eject(path: &Path) -> Result<(), Error> {
    Command::new("hdiutil")
        .arg("eject")
        .arg(path)
        .stdout(Stdio::null())
        .spawn()?
        .wait()?;
    Ok(())
}

pub fn secure_volume(path: &Path, days: u32) -> Result<(), Error> {
    let good_until = (SystemTime::now() + Duration::from_secs((days as u64) * 60 * 60 * 24))
        .duration_since(SystemTime::UNIX_EPOCH)?;
    fs::write(path.join(".metadata_never_index"), "")?;
    fs::write(
        path.join(".encrypted-volume-good-until"),
        good_until.as_secs().to_string(),
    )?;

    // place one of the custom icons shipped with macos as volume icon so it can be easily
    // told apart form other DMGs.
    if fs::copy(
        "/System/Library/CoreServices/CoreTypes.bundle/Contents/Resources/iDiskUserIcon.icns",
        path.join(".VolumeIcon.icns"),
    )
    .is_ok()
    {
        Command::new("SetFile")
            .arg("-a")
            .arg("C")
            .arg(path)
            .spawn()?
            .wait()?;
    }

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
