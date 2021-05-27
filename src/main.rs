use std::{
    fmt::Write,
    fs,
    path::{Path, PathBuf},
    process::Command,
    time::SystemTime,
};

use anyhow::{bail, Error};
use dialoguer::Password;
use structopt::StructOpt;
use uuid::Uuid;
use which::which;

use crate::cli::{
    Commands, CronCommand, EjectCommand, ImageOptions, ImportCommand, ListCommand, NewCommand,
};

mod cli;
mod dmg;
mod zip;

#[derive(Debug)]
struct PrepareResult {
    password: String,
    dmg_path: PathBuf,
    mounted_at: PathBuf,
}

fn prepare_dmg(
    opts: &ImageOptions,
    size: usize,
    source_path: Option<&Path>,
) -> Result<PrepareResult, Error> {
    let password = match opts.password {
        Some(ref password) => password.clone(),
        None => {
            if !opts.keep_dmg && source_path.is_none() {
                Uuid::new_v4().to_simple().to_string()
            } else {
                Password::new().with_prompt("password").interact()?
            }
        }
    };

    let volume_name = match opts.volume_name {
        Some(ref name) => name.as_str(),
        None => source_path
            .and_then(|x| x.file_stem().and_then(|x| x.to_str()))
            .unwrap_or("EncryptedScratchpad"),
    };
    let dmg_path =
        std::env::temp_dir().join(format!("encrypted-{}-{}.dmg", Uuid::new_v4(), volume_name));

    println!("[1] Creating encrypted DMG");
    dmg::make_dmg(&dmg_path, volume_name, size, &password)?;
    println!("[2] Mounting DMG");
    let mounted_at = dmg::mount_dmg(&dmg_path, &password)?;
    println!("[3] Securing mounted volume");
    dmg::secure_volume(&mounted_at, opts.days)?;

    Ok(PrepareResult {
        password,
        dmg_path,
        mounted_at,
    })
}

fn finalize_dmg(opts: &ImageOptions, result: &PrepareResult) -> Result<(), Error> {
    if opts.keep_dmg {
        println!("Placed encrypted DMG at: {}", result.dmg_path.display());
    } else {
        fs::remove_file(&result.dmg_path)?;
    }
    println!("Mounted encrypted DMG at: {}", result.mounted_at.display());
    println!("Ummount with: umount \"{}\"", result.mounted_at.display());
    Ok(())
}

fn new_command(args: NewCommand) -> Result<(), Error> {
    let result = prepare_dmg(&args.image_opts, args.size, None)?;
    finalize_dmg(&args.image_opts, &result)?;
    Ok(())
}

fn import_command(args: ImportCommand) -> Result<(), Error> {
    let input_path = fs::canonicalize(&args.path)?;

    if !fs::metadata(&input_path).map_or(false, |x| x.is_file()) {
        bail!("source archive is not a file");
    }
    let size = zip::get_uncompressed_zip_size(&input_path)? + args.extra_size;
    let result = prepare_dmg(&args.image_opts, size, Some(&input_path))?;
    if !zip::check_password(&input_path, &result.password)? {
        bail!("invalid password");
    }
    println!("[4] Extracting encrypted zip");
    zip::extract(&input_path, &result.mounted_at, &result.password)?;
    finalize_dmg(&args.image_opts, &result)?;
    Ok(())
}

fn list_command(args: ListCommand) -> Result<(), Error> {
    let encrypted_volumes = dmg::list_volumes()?;
    for (mount_point, expires) in encrypted_volumes {
        println!("{}", mount_point.display());
        if args.verbose {
            println!(
                "  expires: {}",
                chrono::DateTime::<chrono::Utc>::from(expires)
            );
        }
    }
    Ok(())
}

fn eject_command(args: EjectCommand) -> Result<(), Error> {
    let encrypted_volumes = dmg::list_volumes()?;
    let reference_path = args.path.as_ref().and_then(|x| fs::canonicalize(x).ok());
    let mut image_found = false;

    let now = SystemTime::now();
    for (mount_point, expires) in encrypted_volumes {
        let expired = expires < now;
        let is_match =
            reference_path.is_some() && fs::canonicalize(&mount_point).ok() == reference_path;
        if (args.expired && expired) || args.all || is_match {
            println!(
                "Ejecting {}volume {}",
                if expired { "expired " } else { "" },
                mount_point.display()
            );
            dmg::eject(&mount_point)?;
        }
        if is_match {
            image_found = true;
        }
    }

    if !image_found && args.path.is_some() {
        bail!("volume was not mounted");
    }

    Ok(())
}

fn cron_command(args: CronCommand) -> Result<(), Error> {
    Command::new("crontab")
        .arg("-e")
        .env(
            "CRONTAB_MODE",
            if args.install { "install" } else { "uninstall" },
        )
        .env("EDITOR", std::env::current_exe()?)
        .spawn()?
        .wait()?;
    Ok(())
}

fn do_cronedit() -> Result<bool, Error> {
    let mut cron = String::new();
    let add = match std::env::var("CRONTAB_MODE").as_deref() {
        Ok("install") => true,
        Ok("uninstall") => false,
        _ => return Ok(false),
    };

    let path = std::env::args_os().nth(1).unwrap();
    let executable = std::env::current_exe()?;
    let cron_cmd = format!("{} eject --expired", executable.display());
    let mut found = false;

    for line in fs::read_to_string(&path)?.lines() {
        if line.trim().ends_with(&cron_cmd) {
            found = true;
            if !add {
                continue;
            }
        }
        writeln!(cron, "{}", line)?;
    }

    if add && !found {
        writeln!(cron, "0 * * * * {}", cron_cmd)?;
    }

    fs::write(&path, cron)?;

    Ok(true)
}

fn main() -> Result<(), Error> {
    if do_cronedit()? {
        return Ok(());
    }

    let commands = Commands::from_args();

    if which("7z").is_err() {
        bail!("7z is not available");
    }

    match commands {
        Commands::New(args) => new_command(args),
        Commands::Import(args) => import_command(args),
        Commands::List(args) => list_command(args),
        Commands::Eject(args) => eject_command(args),
        Commands::Cron(args) => cron_command(args),
    }
}
