// Copyright 2024 the JSR authors. All rights reserved. MIT license.

import { useEffect, useRef } from "preact/hooks";
import { type PackageDownloads, type DownloadDataPoint } from "../../../utils/api_types.ts";

type Props = {
  downloads: PackageDownloads;
};

export function DownloadChart(props: Props) {
  const chartRef = useRef<HTMLDivElement>(null);
  console.log("downloads", props.downloads);
  useEffect(() => {
    // deno-lint-ignore no-explicit-any
    let chart: any;
    (async () => {
      const { default: ApexCharts } = await import("apexcharts");
      const dataPointsWithDownloads = props.downloads.recentVersions.filter((dataPoints) => 
        dataPoints.downloads.length > 0
      );

      const dataPointsToDisplay = dataPointsWithDownloads.slice(0, 5);
      const others = dataPointsWithDownloads.slice(5).map((dataPoints) => dataPoints.downloads).flat();

      const xValues = collectX(dataPointsToDisplay.map((version) => version.downloads));
      chart = new ApexCharts(chartRef.current!, {
        chart: {
          type: "area",
          stacked: true,
          animations: {
            enabled: false,
          },
          height: "100%",
          width: "100%",
        },
        legend: {
          horizontalAlign: "left",
          position: "top",
          showForSingleSeries: true,
        },
        dataLabels: {
          enabled: false
        },
        stroke: {
          curve: "straight",
          width: 1.7,
        },
        series: [...dataPointsToDisplay.map((version) => ({
          name: version.version,
          data: normalize(version.downloads, xValues),
        })), {
          name: "Other",
          data: normalize(others, xValues),
        }],
        xaxis: { type: "datetime" },
      });
      chart.render();
    })();
    return () => {
      chart.destroy();
    };
  }, []);

  return (
    <div class="w-full h-[400px]">
      <div ref={chartRef}></div>
    </div>
  );
}

function collectX(dataPoints: DownloadDataPoint[][]) {
  const xValues = new Set<string>();
  dataPoints.forEach((points) => {
    points.forEach((point) => {
      xValues.add(point.timeBucket);
    });
  });

  return Array.from(xValues).sort((a, b) => new Date(a).getTime() - new Date(b).getTime());
}

function normalize(dataPoints: DownloadDataPoint[], xValues: string[]) {
  const normalized: { [key: string]: number } = {};
  for (const date of xValues) {
    normalized[date] = 0;
  }
  dataPoints.forEach((point) => {
    const key = point.timeBucket;
    if (normalized[key]) {
      normalized[key] += point.count;
    } else {
      normalized[key] = point.count;
    }
  });

  return Object.entries(normalized).map(([key, value]) => ([new Date(key), value]));
}
