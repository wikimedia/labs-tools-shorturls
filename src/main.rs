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

#![feature(proc_macro_hygiene, decl_macro)]

use anyhow::Result;
use chrono::{Date, NaiveDate, TimeZone, Utc};
use flate2::read::GzDecoder;
use rocket::{http::ContentType, response::Content};
use rocket_contrib::json::Json;
use rocket_contrib::templates::{
    tera::{Result as TeraResult, Value},
    Template,
};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, fs, io, io::BufRead, path::PathBuf};
use thousands::Separable;
use url::Url;

#[macro_use]
extern crate rocket;

#[derive(Serialize, Deserialize)]
struct IndexTemplate {
    stats: Vec<DomainInfo>,
    total: i32,
}

#[derive(Serialize, Deserialize)]
struct DomainInfo {
    domain: String,
    count: i32,
}

#[derive(Serialize, Deserialize)]
struct ErrorTemplate {
    error: String,
}

#[get("/")]
fn index() -> Template {
    match build_index() {
        Ok(index) => Template::render("main", index),
        Err(error) => Template::render("error", error),
    }
}

#[get("/api.json")]
fn index_api() -> Result<Json<IndexTemplate>> {
    // FIXME: Error handling
    match build_index() {
        Ok(index) => Ok(Json(index)),
        Err(error) => panic!(error.error),
    }
}

fn build_index() -> Result<IndexTemplate, ErrorTemplate> {
    let latest = match get_latest_dump() {
        Ok(latest) => latest,
        Err(e) => {
            return Err(ErrorTemplate {
                error: e.to_string(),
            })
        }
    };
    match get_info(latest) {
        Ok(info) => Ok(info),
        Err(e) => Err(ErrorTemplate {
            error: e.to_string(),
        }),
    }
}

fn find_dumps() -> Result<Vec<PathBuf>> {
    let mut files: Vec<PathBuf> = fs::read_dir("/public/dumps/public/other/shorturls")?
        .filter(|f| f.is_ok())
        .map(|f| f.unwrap().path())
        .filter(|f| f.to_str().unwrap().ends_with(".gz"))
        .collect();
    files.sort();
    Ok(files)
}

fn get_info(path: PathBuf) -> Result<IndexTemplate> {
    let cache = format!(
        "./cache/{}.cache",
        path.file_name().unwrap().to_str().unwrap()
    );
    //    let cache = format!("{}.cache", path.to_str().unwrap());
    if let Ok(file) = fs::File::open(&cache) {
        return Ok(serde_json::from_reader(file)?);
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
    let mut entries: Vec<DomainInfo> = counts
        .iter()
        .map(|(domain, count)| DomainInfo {
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
    // Save to cache
    serde_json::to_writer(fs::File::create(&cache)?, &index)?;
    Ok(index)
}

fn get_latest_dump() -> Result<PathBuf> {
    Ok(find_dumps()?.pop().unwrap())
}

fn commafy(args: HashMap<String, Value>) -> TeraResult<Value> {
    match args.get("num") {
        Some(val) => Ok(val.separate_with_commas().into()),
        None => Err("No value provided".into()),
    }
}

fn parse_date(fname: &str) -> Result<Date<Utc>> {
    Ok(Utc.from_utc_date(&NaiveDate::parse_from_str(fname, "shorturls-%Y%m%d.gz")?))
}

#[get("/chart.svg")]
fn chart_svg() -> Content<String> {
    Content(ContentType::SVG, chart2().unwrap())
}

fn chart2() -> Result<String> {
    use plotters::prelude::*;
    let mut buf = String::new();
    {
        let root_area = SVGBackend::with_string(&mut buf, (900, 300)).into_drawing_area();
        root_area.fill(&WHITE)?;

        let mut data = Vec::new();
        let mut domain = Vec::new();
        let mut final_total: f32 = 0.0;
        for dump in find_dumps()? {
            let date = parse_date(&dump.file_name().unwrap().to_str().unwrap())?;
            domain.push(date);
            let info = get_info(dump)?;
            data.push((date, info.total as f32));
            final_total = info.total as f32;
        }

        let start_date = domain[0];
        let end_date = domain.last().unwrap();

        let mut ctx = ChartBuilder::on(&root_area)
            .set_label_area_size(LabelAreaPosition::Left, 60)
            .set_label_area_size(LabelAreaPosition::Bottom, 60)
            // Set the y-range from 0 to 105% of max so we don't cut off the top of the chart
            .build_ranged(start_date..*end_date, 0.0..final_total * 1.05)?;

        ctx.configure_mesh()
            .disable_x_mesh()
            .disable_y_mesh()
            .draw()?;

        ctx.draw_series(LineSeries::new(data, &BLUE))?;
    }
    Ok(buf)
}

fn main() {
    rocket::ignite()
        .attach(Template::custom(|engines| {
            engines.tera.register_function("commafy", Box::new(commafy));
        }))
        .mount("/", routes![index, index_api, chart_svg])
        .launch();
}
