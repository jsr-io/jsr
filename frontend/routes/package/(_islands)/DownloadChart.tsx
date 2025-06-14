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

const getChartOptions = (
  isDarkMode: boolean,
  stacked: boolean,
): ApexCharts.ApexOptions => ({
  chart: {
    type: "area",
    stacked,
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
    horizontalAlign: "right",
    offsetY: -1,
    position: "top",
    showForSingleSeries: true,
    labels: {
      colors: isDarkMode ? "#a8b2bd" : "#515d6c", // jsr-gray-300 for dark mode, jsr-gray-600 for light
    },
  },
  tooltip: {
    theme: isDarkMode ? "dark" : "light",
  },
  dataLabels: {
    enabled: false,
  },
  stroke: {
    curve: "straight",
    width: 1.7,
  },
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
        horizontalAlign: "left",
        legend: {
          offsetY: -30,
        },
      },
    },
  ],
});

export function DownloadChart(props: Props) {
  const chartDivRef = useRef<HTMLDivElement>(null);
  const chartRef = useRef<ApexCharts>(null);
  const stackedRef = useRef(true);
  const graphRendered = useSignal(false);

  useEffect(() => {
    (async () => {
      const { default: ApexCharts } = await import("apexcharts");
      const isDarkMode = document.documentElement.classList.contains("dark");

      const initialOptions = getChartOptions(isDarkMode, stackedRef.current);
      initialOptions.series = getSeries(props.downloads, "weekly");
      chartRef.current = new ApexCharts(
        chartDivRef.current!,
        initialOptions,
      );

      chartRef.current.render();
      graphRendered.value = true;

      // Listen for theme changes
      const observer = new MutationObserver(() => {
        const newIsDarkMode = document.documentElement.classList.contains(
          "dark",
        );
        if (chartRef.current) {
          chartRef.current?.updateOptions(
            getChartOptions(newIsDarkMode, stackedRef.current),
          );
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
        <div className="absolute flex top-2 md:-top-4 gap-2 pt-4 text-sm pl-4  z-20">
          <div className="flex items-center gap-2">
            <label htmlFor="aggregationPeriod" className="text-secondary">
              Aggregation Period:
            </label>
            <select
              id="aggregationPeriod"
              onChange={(e) => {
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
              <option value="weekly" selected>
                Weekly
              </option>
              <option value="monthly">Monthly</option>
            </select>
          </div>
          <div className="flex items-center gap-2">
            <label htmlFor="displayAs" className="text-secondary">
              Display As
            </label>
            <select
              id="displayAs"
              onChange={(e) => {
                const newDisplay = e.currentTarget.value === "stacked";
                stackedRef.current = newDisplay;
                // Update chart with new options including the new stacked display
                chartRef.current?.updateOptions(
                  { chart: { stacked: newDisplay } },
                );
              }}
              className="input-container input select w-24"
            >
              <option value="stacked" selected>Stacked</option>
              <option value="unstacked">
                Unstacked
              </option>
            </select>
          </div>
        </div>
      )}
      <style>
        {`
        .apexcharts-legend.apexcharts-align-right.apx-legend-position-top {
          right: unset !important;
        }
        @media (max-width: 598px) {
          .apexcharts-toolbar {
            top: -28px !important;
          }
        }
        @media (min-width: 768px) {
          .apexcharts-legend.apexcharts-align-right.apx-legend-position-top {
            right: 125px !important;
          }
        }
      `}
      </style>
      <div className="h-[300px] md:pt-0 pt-5 text-secondary">
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
      out = new Date(
        Date.UTC(
          date.getUTCFullYear(),
          date.getUTCMonth(),
          date.getUTCDate() - date.getUTCDay(),
        ),
      );
      break;
    case "monthly":
      // first day of month in UTC
      out = new Date(Date.UTC(date.getUTCFullYear(), date.getUTCMonth(), 1));
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

  return Array.from(xValues).sort(
    (a, b) => new Date(a).getTime() - new Date(b).getTime(),
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

  return Object.entries(normalized).map(([key, value]) => [
    new Date(key).getTime(),
    value,
  ]);
}

function getSeries(
  recentVersions: PackageDownloadsRecentVersion[],
  aggregationPeriod: AggregationPeriod,
) {
  const dataPointsWithDownloads = recentVersions.filter(
    (dataPoints) => dataPoints.downloads.length > 0,
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
