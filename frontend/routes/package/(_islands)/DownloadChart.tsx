// Copyright 2024 the JSR authors. All rights reserved. MIT license.

import { useEffect, useRef, useState } from "preact/hooks";
import type {
  DownloadDataPoint,
  PackageDownloadsRecentVersion,
} from "../../../utils/api_types.ts";

interface Props {
  downloads: PackageDownloadsRecentVersion[];
}

export type AggregationPeriod = "daily" | "weekly" | "monthly";

export function DownloadChart(props: Props) {
  const chartDivRef = useRef<HTMLDivElement>(null);
  // deno-lint-ignore no-explicit-any
  const chartRef = useRef<any>(null);
  const [graphRendered, setGraphRendered] = useState(false);

  useEffect(() => {
    (async () => {
      const { default: ApexCharts } = await import("apexcharts");
      chartRef.current = new ApexCharts(chartDivRef.current!, {
        chart: {
          type: "area",
          stacked: true,
          animations: {
            enabled: false,
          },
          height: "100%",
          width: "100%",
          zoom: {
            allowMouseWheelZoom: false,
          },
        },
        legend: {
          horizontalAlign: "center",
          position: "top",
          showForSingleSeries: true,
        },
        tooltip: {
          items: {
            padding: 0,
          },
        },
        dataLabels: {
          enabled: false,
        },
        stroke: {
          curve: "straight",
          width: 1.7,
        },
        series: getSeries(props.downloads, "weekly"),
        xaxis: {
          type: "datetime",
          tooltip: {
            enabled: false,
          },
        },
      });
      chartRef.current.render();
      setGraphRendered(true);
    })();
    return () => {
      chartRef.current.destroy();
      chartRef.current = null;
    };
  }, []);

  return (
    <div class="relative">
      {graphRendered && (
        <div className="absolute flex items-center gap-2 pt-1 text-sm pl-5 z-20">
          <label htmlFor="aggregationPeriod" className="text-gray-700">
            Aggregation Period:
          </label>
          <select
            id="aggregationPeriod"
            onChange={(e) =>
              chartRef.current.updateSeries(
                getSeries(
                  props.downloads,
                  e.currentTarget.value as AggregationPeriod,
                ),
              )}
            className="input-container input px-1.5 py-0.5"
          >
            <option value="daily">Daily</option>
            <option value="weekly" selected>Weekly</option>
            <option value="monthly">Monthly</option>
          </select>
        </div>
      )}
      <div className="h-[300px]">
        <div ref={chartDivRef} />
      </div>
    </div>
  );
}

function formatDateUTC(d: Date): string {
  const yy = d.getUTCFullYear();
  const mm = String(d.getUTCMonth() + 1).padStart(2, "0");
  const dd = String(d.getUTCDate()).padStart(2, "0");
  return `${yy}-${mm}-${dd}`;
}

function adjustTimePeriod(
  timeBucket: string,
  aggregation: AggregationPeriod,
): string {
  const date = new Date(timeBucket);
  if (isNaN(date.getTime())) {
    throw new Error(`Invalid date: ${timeBucket}`);
  }

  let out: Date;
  switch (aggregation) {
    case "weekly":
      // start of week (Sunday) in UTC
      out = new Date(Date.UTC(
        date.getUTCFullYear(),
        date.getUTCMonth(),
        date.getUTCDate() - date.getUTCDay(),
      ));
      break;
    case "monthly":
      // first day of month in UTC
      out = new Date(Date.UTC(
        date.getUTCFullYear(),
        date.getUTCMonth(),
        1,
      ));
      break;
    default: // daily
      out = date;
  }

  return formatDateUTC(out);
}

export function collectX(
  dataPoints: DownloadDataPoint[],
  aggregationPeriod: AggregationPeriod,
) {
  const xValues = new Set<string>();
  dataPoints.forEach((point) => {
    xValues.add(adjustTimePeriod(point.timeBucket, aggregationPeriod));
  });

  return Array.from(xValues).sort((a, b) =>
    new Date(a).getTime() - new Date(b).getTime()
  );
}

export function normalize(
  dataPoints: DownloadDataPoint[],
  xValues: string[],
  aggregationPeriod: AggregationPeriod,
): [Date, number][] {
  const normalized: { [key: string]: number } = {};
  for (const date of xValues) {
    normalized[date] = 0;
  }

  dataPoints.forEach((point) => {
    const key = adjustTimePeriod(point.timeBucket, aggregationPeriod);

    if (normalized[key]) {
      normalized[key] += point.count;
    } else {
      normalized[key] = point.count;
    }
  });

  return Object.entries(normalized).map((
    [key, value],
  ) => [new Date(key), value]);
}

function getSeries(
  recentVersions: PackageDownloadsRecentVersion[],
  aggregationPeriod: AggregationPeriod,
) {
  const dataPointsWithDownloads = recentVersions.filter((dataPoints) =>
    dataPoints.downloads.length > 0
  );

  const dataPointsToDisplay = dataPointsWithDownloads.slice(0, 5);
  const others = dataPointsWithDownloads.slice(5).map((dataPoints) =>
    dataPoints.downloads
  ).flat();

  const xValues = collectX(
    dataPointsToDisplay.map((version) => version.downloads).flat(),
    aggregationPeriod,
  );

  return [
    ...dataPointsToDisplay.map((version) => ({
      name: version.version,
      data: normalize(version.downloads, xValues, aggregationPeriod),
    })),
    {
      name: "Other",
      data: normalize(others, xValues, aggregationPeriod),
      color: "#e7c50b", // jsr-yellow-500
    },
  ];
}
