"""CSV persistence for oscilloscope captures.

The on-disk format is one row per telemetry sample: a leading "time"
column in seconds, then one column per recorded numeric channel. A
sample that did not carry a channel is written as an empty cell and
loads back as None, so sparse channels survive a round trip.
"""

import csv
from pathlib import Path


def save_wave_csv(path, times, channels) -> None:
    keys = list(channels)
    with open(path, "w", encoding="utf-8", newline="") as f:
        writer = csv.writer(f)
        writer.writerow(["time"] + keys)
        for i, t in enumerate(times):
            row = [repr(float(t))]
            for key in keys:
                values = channels[key]
                value = values[i] if i < len(values) else None
                row.append("" if value is None else repr(float(value)))
            writer.writerow(row)


def load_wave_csv(path):
    with open(path, encoding="utf-8") as f:
        reader = csv.reader(f)
        header = next(reader)
        keys = header[1:]
        times = []
        channels = {key: [] for key in keys}
        for row in reader:
            if not row:
                continue
            times.append(float(row[0]))
            for i, key in enumerate(keys):
                cell = row[i + 1] if (i + 1) < len(row) else ""
                channels[key].append(float(cell) if cell != "" else None)
    return times, channels
