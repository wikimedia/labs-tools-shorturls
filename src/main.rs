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

use anyhow::{anyhow, Result};
use chrono::NaiveDate;
use redis::AsyncCommands;
use rocket::serde::{json::Json, Deserialize, Serialize};
use rocket::{http::ContentType, response::content::Custom};
use rocket_dyn_templates::{
    tera::{Result as TeraResult, Value},
    Template,
};
use shorturls::{find_data, DomainTemplate, IndexTemplate};
use std::{collections::HashMap, path::PathBuf};
use thousands::Separable;
use tokio::fs;

#[macro_use]
extern crate rocket;

#[derive(Serialize, Deserialize)]
struct ErrorTemplate {
    error: String,
}

/// Connect to `tools-redis`
fn connect_redis() -> Result<redis::Client> {
    let host = if std::path::Path::new("/etc/wmcs-project").exists() {
        "tools-redis"
    } else {
        "127.0.0.1"
    };
    Ok(redis::Client::open(format!("redis://{}:6379/", host))?)
}

#[get("/")]
async fn index() -> Template {
    match build_index().await {
        Ok(index) => Template::render("main", index),
        Err(err) => {
            dbg!(&err);
            Template::render(
                "error",
                ErrorTemplate {
                    error: err.to_string(),
                },
            )
        }
    }
}

#[get("/<domain>")]
async fn domain(domain: String) -> Template {
    match build_domain(domain).await {
        Ok(dinfo) => Template::render("domain", dinfo),
        Err(error) => Template::render("error", error),
    }
}

/// Build the template for a domain page (e.g. `/query.wikidata.org`)
async fn build_domain(domain: String) -> Result<DomainTemplate, ErrorTemplate> {
    let latest = match get_latest_data() {
        Ok(latest) => latest,
        Err(e) => {
            return Err(ErrorTemplate {
                error: e.to_string(),
            })
        }
    };
    let client = match connect_redis() {
        Ok(client) => client,
        Err(err) => {
            return Err(ErrorTemplate {
                error: format!("redis error: {}", err.to_string()),
            });
        }
    };
    match get_data(latest, &client).await {
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
async fn index_api() -> Json<IndexTemplate> {
    // FIXME: Error handling
    match build_index().await {
        Ok(index) => Json(index),
        Err(error) => panic!("{}", error),
    }
}

#[get("/<domain>/api.json")]
async fn domain_api(domain: String) -> Json<DomainTemplate> {
    // FIXME: Error handling
    match build_domain(domain).await {
        Ok(dinfo) => Json(dinfo),
        Err(error) => panic!("{}", error.error),
    }
}

/// Build the index template (`/`)
async fn build_index() -> Result<IndexTemplate> {
    let latest = get_latest_data()?;
    let client = connect_redis()?;
    get_data(latest, &client).await
}

/// get filename for the most recent data file
fn get_latest_data() -> Result<PathBuf> {
    find_data()?
        .pop()
        .ok_or_else(|| anyhow!("Could not find latest data"))
}

/// Get the data out of a data file, caching it in Redis if necessary
async fn get_data(path: PathBuf, client: &redis::Client) -> Result<IndexTemplate> {
    let cache_key = format!("shorturls:{}", path.to_str().unwrap());
    let data = match client.get_async_connection().await {
        Ok(mut conn) => {
            let info: Option<String> = conn.get(&cache_key).await?;
            if let Some(json) = info {
                // If we can deserialize it, return , otherwise we'll just reread
                // it from disk
                if let Ok(val) = serde_json::from_str(&json) {
                    return Ok(val);
                }
            }

            let data: IndexTemplate = serde_json::from_str(&fs::read_to_string(&path).await?)?;

            // Cache for 30 days
            conn.set_ex(&cache_key, serde_json::to_string(&data)?, 60 * 60 * 24 * 30)
                .await?;

            data
        }
        // Couldn't connect to redis, run without caching
        Err(err) => {
            dbg!(&err);
            // XXX: Can we avoid duplication here?
            serde_json::from_str(&fs::read_to_string(&path).await?)?
        }
    };

    Ok(data)
}

/// tera template helper to stick commas into large numbers
fn commafy(args: &HashMap<String, Value>) -> TeraResult<Value> {
    match args.get("num") {
        Some(val) => Ok(val.separate_with_commas().into()),
        None => Err("No value provided".into()),
    }
}

/// parse the date out of data file names
fn parse_date(fname: &str) -> Result<NaiveDate> {
    Ok(NaiveDate::parse_from_str(
        fname,
        "shorturls-%Y%m%d.gz.data",
    )?)
}

#[get("/chart.svg")]
async fn chart_svg() -> Custom<String> {
    Custom(ContentType::SVG, chart2(None).await.unwrap())
}

#[get("/<domain>/chart.svg")]
async fn domain_chart_svg(domain: String) -> Custom<String> {
    Custom(ContentType::SVG, chart2(Some(&domain)).await.unwrap())
}

/// Generate an SVG chart
async fn chart2(domain: Option<&str>) -> Result<String> {
    use plotters::prelude::*;
    let mut buf = String::new();
    {
        let client = connect_redis()?;

        let mut datapoints = Vec::new();
        let mut domainpoints = Vec::new();
        let mut chart_domain = Vec::new();
        let mut final_total: f32 = 0.0;
        for data in find_data()? {
            let date = parse_date(&data.file_name().unwrap().to_str().unwrap())?;
            chart_domain.push(date);
            let info = get_data(data, &client).await?;
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

        let root_area = SVGBackend::with_string(&mut buf, (900, 300)).into_drawing_area();
        root_area.fill(&WHITE)?;
        let mut ctx = ChartBuilder::on(&root_area)
            .set_label_area_size(LabelAreaPosition::Left, 60)
            .set_label_area_size(LabelAreaPosition::Bottom, 60)
            // Set the y-range from 0 to 105% of max so we don't cut off the top of the chart
            .build_cartesian_2d(start_date..*end_date, 0.0..final_total * 1.05)?;

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

#[get("/healthz")]
fn healthz() -> &'static str {
    "OK"
}

#[launch]
fn rocket() -> _ {
    rocket::build()
        .attach(Template::custom(|engines| {
            engines.tera.register_function("commafy", Box::new(commafy));
        }))
        .mount(
            "/",
            routes![
                index,
                index_api,
                chart_svg,
                domain,
                domain_api,
                domain_chart_svg,
                healthz,
            ],
        )
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_commafy() {
        let mut map: HashMap<String, Value> = HashMap::new();
        map.insert("num".to_string(), Value::String("9999999".to_string()));
        let result = commafy(&map);
        assert_eq!(Value::String("\"9,999,999\"".to_string()), result.unwrap());
    }
}
