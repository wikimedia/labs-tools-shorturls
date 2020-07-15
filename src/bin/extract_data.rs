/*
Statistics about w.wiki
Copyright (C) 2020 Kunal Mehta <legoktm@member.fsf.org>

This program is free software: you can redistribute it and/or modify
it under the terms of the GNU Affero General Public License as published by
the Free Software Foundation, either version 3 of the License, or
(at your option) any later version.

This program is distributed in the hope that it will be useful,
but WITHOUT ANY WARRANTY; without even the implied warranty of
MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
GNU Affero General Public License for more details.

You should have received a copy of the GNU Affero General Public License
along with this program.  If not, see <http://www.gnu.org/licenses/>.
*/

use anyhow::Result;
use flate2::read::GzDecoder;
use shorturls::{DomainTemplate, IndexTemplate};
use std::{collections::HashMap, fs, io, io::BufRead, path::PathBuf};
use url::Url;

fn find_dumps() -> Result<Vec<PathBuf>> {
    let mut files: Vec<PathBuf> = fs::read_dir("/public/dumps/public/other/shorturls")?
        .filter(|f| f.is_ok())
        .map(|f| f.unwrap().path())
        .filter(|f| f.to_str().unwrap().ends_with(".gz"))
        .collect();
    files.sort();
    Ok(files)
}

fn save_dump(path: PathBuf) -> Result<()> {
    let data = format!(
        "./data/{}.data",
        path.file_name().unwrap().to_str().unwrap()
    );
    if std::path::Path::new(&data).exists() {
        return Ok(());
    }
    let gz = GzDecoder::new(fs::File::open(path)?);
    let buffered = io::BufReader::new(gz);
    let mut counts: HashMap<String, i32> = HashMap::new();
    for rline in buffered.lines() {
        let line = rline?;
        let sp: Vec<&str> = line.splitn(2, '|').collect();
        let parsed = match Url::parse(sp[1]) {
            Ok(url) => url,
            // In theory this shouldn't be possible since UrlShortener
            // should validate URLs, but it happens. TODO: Report this
            // upstream...to me.
            Err(_) => {
                continue;
            }
        };
        let domain = match parsed.host_str() {
            Some(domain) => domain.to_string(),
            None => {
                continue;
            }
        };
        let counter = counts.entry(domain).or_insert(0);
        *counter += 1;
    }
    let mut entries: Vec<DomainTemplate> = counts
        .iter()
        .map(|(domain, count)| DomainTemplate {
            domain: domain.to_string(),
            count: *count,
        })
        .collect();
    let mut total: i32 = 0;
    for entry in &entries {
        total += entry.count;
    }
    entries.sort_by(|a, b| b.count.cmp(&a.count));
    let index = IndexTemplate {
        stats: entries,
        total,
    };
    // Save to data file
    println!("Writing to {}", data);
    serde_json::to_writer(fs::File::create(&data)?, &index)?;
    Ok(())
}

fn main() -> Result<()> {
    for dump in find_dumps()? {
        save_dump(dump)?
    }
    Ok(())
}
