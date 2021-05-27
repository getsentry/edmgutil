use std::{
    path::Path,
    process::{Command, Stdio},
};

use anyhow::Error;

pub fn get_uncompressed_zip_size(path: &Path) -> Result<usize, Error> {
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

pub fn check_password(path: &Path, password: &str) -> Result<bool, Error> {
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

pub fn extract(src: &Path, dst: &Path, password: &str) -> Result<(), Error> {
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
