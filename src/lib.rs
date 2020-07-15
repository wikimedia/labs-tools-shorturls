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
use serde::{Deserialize, Serialize};
use std::{fs, path::PathBuf};

#[derive(Serialize, Deserialize)]
pub struct IndexTemplate {
    pub stats: Vec<DomainInfo>,
    pub total: i32,
}

#[derive(Serialize, Deserialize)]
pub struct DomainInfo {
    pub domain: String,
    pub count: i32,
}

pub fn find_data() -> Result<Vec<PathBuf>> {
    let mut files: Vec<PathBuf> = fs::read_dir("./data")?
        .filter(|f| f.is_ok())
        .map(|f| f.unwrap().path())
        .filter(|f| f.to_str().unwrap().ends_with(".data"))
        .collect();
    files.sort();
    Ok(files)
}
