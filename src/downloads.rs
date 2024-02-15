use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::Error;
use url::Url;

pub fn find_downloads_in_folder(
    download_dir: &Path,
    is_match: impl Fn(&Url, &Path) -> bool,
) -> Result<Vec<(PathBuf, Url)>, Error> {
    let mut matches = vec![];

    for entry in fs::read_dir(download_dir)? {
        let entry = entry?;
        let attr = xattr::get(entry.path(), "com.apple.metadata:kMDItemWhereFroms");
        if let Ok(Some(encoded_plist)) = attr {
            let might_be_urls: Vec<String> = plist::from_bytes(&encoded_plist)?;
            let parsed_urls = might_be_urls
                .into_iter()
                .filter_map(|x| Url::parse(&x).ok())
                .collect::<Vec<_>>();
            if let Some(source) = parsed_urls
                .into_iter()
                .filter(|url| is_match(&url, &entry.path()))
                .next()
            {
                matches.push((entry.path().to_owned(), source));
            }
        }
    }

    matches.sort_by_cached_key(|x| x.0.file_name().map(|x| x.to_owned()));

    Ok(matches)
}
