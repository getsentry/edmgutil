# edmgutil

`edmgutil` is a simple wrapper utility to hdiutil to help you work with disposable, encrypted
DMGs. It can decompress an encrypted ZIP into a newly mounted encrypted DMG, create empty
throwaway DMGs and automatically eject expired ones. This makes transferring and working with
data that should only live for a short period of time for debugging purposes to developer
machines a more convenient endeavour. The volume is individually encrypted and gets destroyed
when ejected.

It also instructs the backup tool to disable backing up the volume in case someone accidentally
adds it.

## Installation

```
cargo install --git https://github.com/getsentry/edmgutil --branch main edmgutil
```

Note that this requires `7z` to be installed. If you don't have it:

```
brew install p7zip
```

## Importing Encrypted Zip Archives

```
edmgutil import /path/to/encrypted.zip
```

It will prompt for the password, then create an encrypted volumne with the same password and then
extract the zip file into it and then delete the created dmg (unless `-k` is passed).

Once the DMG is ejected everything is gone again.

When the DMG is created a timestamp is frozen into it (defined by `--days`, defaults to 7). It's
recommended to run `edmgutil eject --expired` regularly to automatically unmount expired
images for instance by putting it into your crontab (see `edmgutil cron`).

To create an encrypted zip use 7zip:

```
7za a -tzip -p'the password' -mem=AES256 encrypted.zip folder
```

Just make sure to use a long password, maybe something like this:

```
openssl rand -hex 32
```

## Creating Empty DMGs

To create an empty, encrypted DMG use the `new` command and provide the size of the DMG in
megabytes. Alternatively you can provide a descriptive name which will become the volume name:

```
edmgutil new --size 100 --name "My Stuff"
```

## Listing / Ejecting

To list and eject encrypted DMGs you can use the following commands:

```
edmgutil list
edmgutil eject --expired
edmgutil eject --all
edmgutil eject /Volumes/EncryptedVolume
```

## Crontab

To ensure that expired images are ejected automatically when possible can can install a crontab
which runs ejecting hourly:

```
edmgutil cron --install
```

## Download Folder Monitoring

Because browsers love to download files unprompted into the default download location it's not
uncommon for you to accidentally places files there you really don't want to retain there.
The `find-downloads` command can be useful for manual spot checking.

This will list all files that were downloaded from `your-domain.tld`:

```
edmgutil find-downloads -d your-domain.tld
```

For additional information you can turn on verbose mode which shows the exact source of the
file by URL:

```
edmgutil find-downloads -d your-domain.tld -v
```
