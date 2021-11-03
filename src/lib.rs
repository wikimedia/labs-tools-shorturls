/*
Statistics about w.wiki
Copyright (C) 2020 Kunal Mehta <legoktm@debian.org>

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

//! A [Toolforge tool](https://shorturls.toolforge.org/) to generate statistics
//! for the [w.wiki](https://w.wiki/) URL shortener.
//!
//! The `extract_data` cron job parses dumps into JSON data files so we can generate
//! historical comparisons and charts. The `shorturls` webserver reads from the data files,
//! which are cached in Redis for extra performance, and serves a webserver with HTML output
//! and corresponding API endpoints.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::{fs, path::PathBuf};

/// Tera template for the index, but also the structure of data files
#[derive(Serialize, Deserialize)]
pub struct IndexTemplate {
    pub stats: Vec<DomainTemplate>,
    pub total: i32,
}

/// Tera template for domain pages
#[derive(Serialize, Deserialize)]
pub struct DomainTemplate {
    pub domain: String,
    pub count: i32,
}

/// Get a sorted list of all the data files
pub fn find_data() -> Result<Vec<PathBuf>> {
    let mut files: Vec<PathBuf> = fs::read_dir("./data")?
        // TODO: use filter_map
        .filter(|f| f.is_ok())
        .map(|f| f.unwrap().path())
        .filter(|f| f.to_str().unwrap().ends_with(".data"))
        .collect();
    files.sort();
    Ok(files)
}
