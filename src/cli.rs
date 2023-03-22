use std::path::PathBuf;

use clap::AppSettings;
use structopt::StructOpt;

/// A utility to work with disposable encryptd DMGs.
#[derive(Debug, StructOpt)]
#[structopt(
    global_setting(AppSettings::UnifiedHelpMessage),
    global_setting(AppSettings::VersionlessSubcommands)
)]
pub enum Commands {
    New(NewCommand),
    Import(ImportCommand),
    List(ListCommand),
    Eject(EjectCommand),
    Cron(CronCommand),
    FindDownloads(FindDownloadsCommand),
}

#[derive(Debug, StructOpt)]
pub struct ImageOptions {
    /// the amount of days the image is good to keep
    #[structopt(long = "days", default_value = "7")]
    pub days: u32,
    /// the volume name of the dmg
    #[structopt(short = "n", long = "name")]
    pub volume_name: Option<String>,
    /// keep the source DMG instead of deleting it
    #[structopt(short = "k", long = "keep")]
    pub keep_dmg: bool,
    /// provide the passphrase for the image
    #[structopt(short = "p", long = "password")]
    pub password: Option<String>,
}

/// creates a new encrypted DMG and mounts it
///
/// This command can create an encrypted DMG, mounts it and normally
/// disposes of the source DMG so that everything gets deleted when
/// the image is unmounted.
#[derive(Debug, StructOpt)]
pub struct NewCommand {
    #[structopt(flatten)]
    pub image_opts: ImageOptions,
    /// the size for the encrypted DMG in megabytes
    #[structopt(short = "s", long = "size", default_value = "100")]
    pub size: usize,
}

/// imports an encrypted zip as encrypted DMG and mounts it
#[derive(Debug, StructOpt)]
pub struct ImportCommand {
    #[structopt(flatten)]
    pub image_opts: ImageOptions,
    /// the extra size for the encrypted DMG in megabytes
    #[structopt(long = "extra-size", default_value = "100")]
    pub extra_size: usize,
    /// the path of the input zip archive
    #[structopt(name = "path")]
    pub path: PathBuf,
}

/// ejects encrypted dmgs
#[derive(Debug, StructOpt)]
#[structopt(setting(AppSettings::ArgRequiredElseHelp))]
pub struct EjectCommand {
    /// ejects all mounted encrypted volumes
    #[structopt(long = "all", short = "a", conflicts_with("path"))]
    pub all: bool,
    /// ejects expired encrypted volumes
    #[structopt(long = "expired", short = "e", conflicts_with("path"))]
    pub expired: bool,
    /// the path of the volume to eject
    #[structopt(name = "path")]
    pub path: Option<PathBuf>,
}

/// list all mounted encrypted DMGs
#[derive(Debug, StructOpt)]
pub struct ListCommand {
    /// provides extra information
    #[structopt(long = "verbose", short = "v")]
    pub verbose: bool,
}

/// installs or uninstalls the cron
#[derive(Debug, StructOpt)]
#[structopt(setting(AppSettings::ArgRequiredElseHelp))]
pub struct CronCommand {
    /// installs the cron
    #[structopt(long = "install")]
    pub install: bool,
    /// uninstalls the cron
    #[structopt(long = "uninstall")]
    pub uninstall: bool,
}

/// helps monitoring the download folder for problematic sources
///
/// This command lets you quickly show all the files in your downloads folder
/// which come from a specific source.  This way you can easily periodically
/// check that it does not contain files you don't expect it to be there.
#[derive(Debug, StructOpt)]
pub struct FindDownloadsCommand {
    /// the domains to look out for.
    ///
    /// When *.domain.tld is used it looks for any subdomain of the domain
    /// including the apex of the domain.
    #[structopt(long = "domain", short = "d")]
    pub domains: Vec<String>,
    /// provide additional information when listing files
    #[structopt(long = "verbose", short = "v")]
    pub verbose: bool,
    /// an alternative folder than the default download folder to search
    pub path: Option<PathBuf>,
    /// Automatically delete all found files.
    #[structopt(long = "delete")]
    pub delete: bool,
    /// Only list or delete files older than this value.
    #[structopt(long = "days")]
    pub days: Option<u32>,
}
