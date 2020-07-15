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
use rocket::{http::ContentType, response::Content};
use rocket_contrib::databases::{redis, redis::Commands};
use rocket_contrib::json::Json;
use rocket_contrib::templates::{
    tera::{Result as TeraResult, Value},
    Template,
};
use serde::{Deserialize, Serialize};
use shorturls::{find_data, DomainTemplate, IndexTemplate};
use std::{collections::HashMap, fs, path::PathBuf};
use thousands::Separable;

#[macro_use]
extern crate rocket;

#[macro_use]
extern crate rocket_contrib;

#[database("cache")]
struct RedisCache(redis::Connection);

#[derive(Serialize, Deserialize)]
struct ErrorTemplate {
    error: String,
}

#[get("/")]
fn index(conn: RedisCache) -> Template {
    match build_index(conn) {
        Ok(index) => Template::render("main", index),
        Err(error) => Template::render("error", error),
    }
}

#[get("/<domain>")]
fn domain(domain: String, conn: RedisCache) -> Template {
    match build_domain(domain, conn) {
        Ok(dinfo) => Template::render("domain", dinfo),
        Err(error) => Template::render("error", error),
    }
}

fn build_domain(domain: String, conn: RedisCache) -> Result<DomainTemplate, ErrorTemplate> {
    let latest = match get_latest_data() {
        Ok(latest) => latest,
        Err(e) => {
            return Err(ErrorTemplate {
                error: e.to_string(),
            })
        }
    };
    match get_data(latest, &conn) {
        Ok(info) => {
            for dinfo in info.stats {
                if dinfo.domain == domain {
                    return Ok(dinfo);
                }
            }
            Err(ErrorTemplate {
                error: "Unknown domain specified".to_string(),
            })
        }
        Err(e) => Err(ErrorTemplate {
            error: e.to_string(),
        }),
    }
}

#[get("/api.json")]
fn index_api(conn: RedisCache) -> Result<Json<IndexTemplate>> {
    // FIXME: Error handling
    match build_index(conn) {
        Ok(index) => Ok(Json(index)),
        Err(error) => panic!(error.error),
    }
}

#[get("/<domain>/api.json")]
fn domain_api(domain: String, conn: RedisCache) -> Result<Json<DomainTemplate>> {
    // FIXME: Error handling
    match build_domain(domain, conn) {
        Ok(dinfo) => Ok(Json(dinfo)),
        Err(error) => panic!(error.error),
    }
}

fn build_index(conn: RedisCache) -> Result<IndexTemplate, ErrorTemplate> {
    let latest = match get_latest_data() {
        Ok(latest) => latest,
        Err(e) => {
            return Err(ErrorTemplate {
                error: e.to_string(),
            })
        }
    };
    match get_data(latest, &conn) {
        Ok(info) => Ok(info),
        Err(e) => Err(ErrorTemplate {
            error: e.to_string(),
        }),
    }
}

fn get_latest_data() -> Result<PathBuf> {
    Ok(find_data()?.pop().unwrap())
}

fn get_data(path: PathBuf, conn: &RedisCache) -> Result<IndexTemplate> {
    let cache_key = format!("shorturls:{}", path.to_str().unwrap());
    let info: Option<String> = conn.get(&cache_key)?;
    if let Some(json) = info {
        // If we can deserialize it, return , otherwise we'll just reread
        // it from disk
        if let Ok(val) = serde_json::from_str(&json) {
            return Ok(val);
        }
    }
    let data: IndexTemplate = serde_json::from_reader(fs::File::open(&path)?)?;
    // Cache for 30 days
    conn.set_ex(&cache_key, serde_json::to_string(&data)?, 60 * 60 * 24 * 30)?;

    Ok(data)
}

fn commafy(args: HashMap<String, Value>) -> TeraResult<Value> {
    match args.get("num") {
        Some(val) => Ok(val.separate_with_commas().into()),
        None => Err("No value provided".into()),
    }
}

fn parse_date(fname: &str) -> Result<Date<Utc>> {
    Ok(Utc.from_utc_date(&NaiveDate::parse_from_str(
        fname,
        "shorturls-%Y%m%d.gz.data",
    )?))
}

#[get("/chart.svg")]
fn chart_svg(conn: RedisCache) -> Content<String> {
    Content(ContentType::SVG, chart2(conn, None).unwrap())
}

#[get("/<domain>/chart.svg")]
fn domain_chart_svg(domain: String, conn: RedisCache) -> Content<String> {
    Content(ContentType::SVG, chart2(conn, Some(&domain)).unwrap())
}

fn chart2(conn: RedisCache, domain: Option<&str>) -> Result<String> {
    use plotters::prelude::*;
    let mut buf = String::new();
    {
        let root_area = SVGBackend::with_string(&mut buf, (900, 300)).into_drawing_area();
        root_area.fill(&WHITE)?;

        let mut datapoints = Vec::new();
        let mut domainpoints = Vec::new();
        let mut chart_domain = Vec::new();
        let mut final_total: f32 = 0.0;
        for data in find_data()? {
            let date = parse_date(&data.file_name().unwrap().to_str().unwrap())?;
            chart_domain.push(date);
            let info = get_data(data, &conn)?;
            datapoints.push((date, info.total as f32));
            final_total = info.total as f32;
            if let Some(host) = domain {
                for dinfo in info.stats {
                    if dinfo.domain == host {
                        domainpoints.push((date, dinfo.count as f32));
                        break;
                    }
                }
            }
        }

        let start_date = chart_domain[0];
        let end_date = chart_domain.last().unwrap();

        let mut ctx = ChartBuilder::on(&root_area)
            .set_label_area_size(LabelAreaPosition::Left, 60)
            .set_label_area_size(LabelAreaPosition::Bottom, 60)
            // Set the y-range from 0 to 105% of max so we don't cut off the top of the chart
            .build_ranged(start_date..*end_date, 0.0..final_total * 1.05)?;

        ctx.configure_mesh()
            .disable_x_mesh()
            .disable_y_mesh()
            .draw()?;

        ctx.draw_series(LineSeries::new(datapoints, &BLUE))?;

        if domain.is_some() && !domainpoints.is_empty() {
            ctx.draw_series(LineSeries::new(domainpoints, &GREEN))?;
        }
    }
    Ok(buf)
}

fn main() {
    rocket::ignite()
        .attach(Template::custom(|engines| {
            engines.tera.register_function("commafy", Box::new(commafy));
        }))
        .attach(RedisCache::fairing())
        .mount(
            "/",
            routes![
                index,
                index_api,
                chart_svg,
                domain,
                domain_api,
                domain_chart_svg
            ],
        )
        .launch();
}
