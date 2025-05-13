// Copyright 2024 the JSR authors. All rights reserved. MIT license.

import { useEffect, useRef } from "preact/hooks";
import type {
  DownloadDataPoint,
  PackageDownloadsRecentVersion,
} from "../../../utils/api_types.ts";
import type ApexCharts from "apexcharts";
import { useSignal } from "@preact/signals";

interface Props {
  downloads: PackageDownloadsRecentVersion[];
}

export type AggregationPeriod = "daily" | "weekly" | "monthly";

export function DownloadChart(props: Props) {
  const chartDivRef = useRef<HTMLDivElement>(null);
  const chartRef = useRef<ApexCharts>(null);
  const graphRendered = useSignal(false);

  const getChartOptions = (
    isDarkMode: boolean,
    aggregationPeriod: AggregationPeriod = "weekly",
  ) => ({
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
      background: "transparent",
      foreColor: isDarkMode ? "#a8b2bd" : "#515d6c", // jsr-gray-300 for dark mode, jsr-gray-600 for light
    },
    legend: {
      horizontalAlign: "center",
      position: "top",
      showForSingleSeries: true,
      labels: {
        colors: isDarkMode ? "#a8b2bd" : "#515d6c", // jsr-gray-300 for dark mode, jsr-gray-600 for light
      },
    },
    tooltip: {
      items: {
        padding: 0,
      },
      theme: isDarkMode ? "dark" : "light",
    },
    dataLabels: {
      enabled: false,
    },
    stroke: {
      curve: "straight",
      width: 1.7,
    },
    series: getSeries(props.downloads, aggregationPeriod),
    xaxis: {
      type: "datetime",
      tooltip: {
        enabled: false,
      },
      labels: {
        style: {
          colors: isDarkMode ? "#ced3da" : "#515d6c", // jsr-gray-200 for dark mode, jsr-gray-600 for light
        },
      },
      axisBorder: {
        color: isDarkMode ? "#47515c" : "#ced3da", // jsr-gray-700 for dark mode, jsr-gray-200 for light
      },
      axisTicks: {
        color: isDarkMode ? "#47515c" : "#ced3da", // jsr-gray-700 for dark mode, jsr-gray-200 for light
      },
    },
    yaxis: {
      labels: {
        style: {
          colors: isDarkMode ? "#a8b2bd" : "#515d6c", // jsr-gray-300 for dark mode, jsr-gray-600 for light
        },
      },
    },
    grid: {
      borderColor: isDarkMode ? "#47515c" : "#e5e8eb", // jsr-gray-700 for dark mode, jsr-gray-100 for light
      strokeDashArray: 3,
    },
    responsive: [
      {
        breakpoint: 768,
        options: {
          legend: {
            horizontalAlign: "left",
          },
        },
      },
    ],
  });

  useEffect(() => {
    (async () => {
      const { default: ApexCharts } = await import("apexcharts");
      const isDarkMode = document.documentElement.classList.contains("dark");

      chartRef.current = new ApexCharts(
        chartDivRef.current!,
        getChartOptions(isDarkMode),
      );

      chartRef.current.render();
      graphRendered.value = true;

      // Listen for theme changes
      const observer = new MutationObserver(() => {
        const newIsDarkMode = document.documentElement.classList.contains(
          "dark",
        );
        if (chartRef.current) {
          chartRef.current?.updateOptions(getChartOptions(newIsDarkMode));
        }
      });

      observer.observe(document.documentElement, {
        attributes: true,
        attributeFilter: ["class"],
      });

      return () => {
        observer.disconnect();
        chartRef.current?.destroy();
        chartRef.current = null;
      };
    })();
  }, []);

  return (
    <div class="relative">
      {graphRendered.value && (
        <div className="absolute flex items-center gap-2 pt-1 text-sm pl-5 z-20">
          <label htmlFor="aggregationPeriod" className="text-secondary">
            Aggregation Period:
          </label>
          <select
            id="aggregationPeriod"
            onChange={(e) => {
              const isDarkMode = document.documentElement.classList.contains(
                "dark",
              );
              const newAggregationPeriod = e.currentTarget
                .value as AggregationPeriod;

              // Update chart with new options including the new aggregation period
              chartRef.current?.updateOptions(
                getChartOptions(isDarkMode, newAggregationPeriod),
              );

              chartRef.current?.updateSeries(
                getSeries(
                  props.downloads,
                  e.currentTarget.value as AggregationPeriod,
                ),
              );
            }}
            className="input-container input select w-20"
          >
            <option value="daily">Daily</option>
            <option value="weekly" selected>Weekly</option>
            <option value="monthly">Monthly</option>
          </select>
        </div>
      )}
      <div className="h-[300px] md:pt-0 pt-10 text-secondary">
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
): [number, number][] {
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
  ) => [new Date(key).getTime(), value]);
}

function getSeries(
  recentVersions: PackageDownloadsRecentVersion[],
  aggregationPeriod: AggregationPeriod,
) {
  const dataPointsWithDownloads = recentVersions.filter((dataPoints) =>
    dataPoints.downloads.length > 0
  );

  const dataPointsToDisplay = dataPointsWithDownloads.slice(0, 5);
  // const others = dataPointsWithDownloads.slice(5).map((dataPoints) =>
  //   dataPoints.downloads
  // ).flat();

  const xValues = collectX(
    dataPointsWithDownloads.map((version) => version.downloads).flat(),
    aggregationPeriod,
  );

  return [
    ...dataPointsToDisplay.map((version) => ({
      name: version.version,
      data: normalize(version.downloads, xValues, aggregationPeriod),
    })),
    // {
    //   name: "Other",
    //   data: normalize(others, xValues, aggregationPeriod),
    //   color: "#e7c50b", // jsr-yellow-500
    // },
  ];
}
