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
use rocket_contrib::json::Json;
use rocket_contrib::templates::{
    tera::{Result as TeraResult, Value},
    Template,
};
use serde::{Deserialize, Serialize};
use shorturls::{find_data, get_data, IndexTemplate};
use std::{collections::HashMap, path::PathBuf};
use thousands::Separable;

#[macro_use]
extern crate rocket;

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
    let latest = match get_latest_data() {
        Ok(latest) => latest,
        Err(e) => {
            return Err(ErrorTemplate {
                error: e.to_string(),
            })
        }
    };
    match get_data(latest) {
        Ok(info) => Ok(info),
        Err(e) => Err(ErrorTemplate {
            error: e.to_string(),
        }),
    }
}

fn get_latest_data() -> Result<PathBuf> {
    Ok(find_data()?.pop().unwrap())
}

fn commafy(args: HashMap<String, Value>) -> TeraResult<Value> {
    match args.get("num") {
        Some(val) => Ok(val.separate_with_commas().into()),
        None => Err("No value provided".into()),
    }
}

fn parse_date(fname: &str) -> Result<Date<Utc>> {
    Ok(Utc.from_utc_date(&NaiveDate::parse_from_str(fname, "shorturls-%Y%m%d.gz.data")?))
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

        let mut datapoints = Vec::new();
        let mut domain = Vec::new();
        let mut final_total: f32 = 0.0;
        for data in find_data()? {
            let date = parse_date(&data.file_name().unwrap().to_str().unwrap())?;
            domain.push(date);
            let info = get_data(data)?;
            datapoints.push((date, info.total as f32));
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

        ctx.draw_series(LineSeries::new(datapoints, &BLUE))?;
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
