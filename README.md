# ezip2dmg

Simple wrapper utility to decompress an encrypted ZIP into a newly mounted encrypted DMG.  This
makes transferring data that should only live for a short period of time for debugging purposes
to developer machines a more convenient endeavour.  The volume is individually encrypted and
when unmounted is lost.

It also instructs the backup tool to disable backing up the volume in case someone accidentally
adds it.  In addition it reserves some extra space on the DMG for experimentation.

## Installation

```
cargo install --git https://github.com/getsentry/ezip2dmg --branch main ezip2dmg
```

Note that this requires `7z` to be installed. If you don't have it:

```
brew install p7zip
```

## Usage

```
ezip2dmg /path/to/encrypted.zip
```

It will prompt for the password, then create an encrypted volumne with the same password and then
extract the zip file into it and then delete the created dmg (unless `-k` is passed).

Once the DMG is unmounted everything is gone again.

When the DMG is created a timestamp is frozen into it (defined by `--days`, defaults to 2).  It's
recommended to run `ezip2dmg --unmount-expired` regularly to automatically unmount expired
images.

## Creating Encrypted Zips

To create an encrypted zip use 7zip:

```
7za a -tzip -p'the password' -mem=AES256 encrypted.zip folder
```

Just make sure to use a long password, maybe something like this:

```
openssl rand -hex 32
```
