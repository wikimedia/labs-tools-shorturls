#!/usr/bin/env python3
"""
Statistics about w.wiki
Copyright (C) 2019 Kunal Mehta <legoktm@member.fsf.org>

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
"""

from collections import defaultdict
from datetime import datetime
from flask import Flask, render_template, make_response
from flask_bootstrap import Bootstrap
import gzip
from io import BytesIO
import json
import matplotlib; matplotlib.use('svg')  # noqa
import matplotlib.pyplot as plot
from pathlib import Path
from urllib.parse import urlparse

app = Flask(__name__)
Bootstrap(app)
DUMPS = Path('/public/dumps/public/other/shorturls')
CACHE = Path(__file__).parent / 'cache'


@app.context_processor
def inject_to_templates():
    return {
        'enumerate': enumerate,
        'format': format,
    }


@app.route('/')
def main():
    stats = read_dump(latest_dump())
    total = stats.pop('total')
    stats = sorted(stats.items(), key=lambda x: x[1], reverse=True)
    return render_template('main.html', stats=stats, total=total)


def list_dumps():
    return list(sorted(DUMPS.glob('shorturls-*.gz')))


def latest_dump():
    return list_dumps()[-1]


def cached_name(path):
    return CACHE / (path.name + '.json')


def read_dump(path):
    cache = cached_name(path)
    if cache.exists():
        with open(str(cache)) as f:
            return json.load(f)
    data = defaultdict(int)
    with gzip.open(str(path), 'rb') as f:
        text = f.read().decode()
    for line in text.splitlines():
        code, url = line.split('|', 1)
        parsed = urlparse(url)
        data[parsed.netloc] += 1

    total = sum(data.values())
    data['total'] = total
    with open(str(cache), 'w') as f:
        json.dump(data, f)

    return data


@app.route('/chart.svg')
def chart():
    x = []
    y = []
    for dump in list_dumps():
        datepart = dump.name[10:-3]
        date = datetime.strptime(datepart, '%Y%m%d')
        total = read_dump(dump)['total']
        x.append(date)
        y.append(total)

    plot.figure(figsize=(16, 4))
    plot.plot(x, y)
    plot.xlabel('Date')
    plot.ylabel('Shortened URLs')
    f = BytesIO()
    plot.savefig(f, format='svg')
    plot.clf()
    plot.cla()
    plot.close()
    resp = make_response(f.getvalue())
    resp.headers['content-type'] = 'image/svg+xml'
    return resp


if __name__ == '__main__':
    app.run(debug=True)
