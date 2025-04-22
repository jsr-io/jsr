// Copyright 2024 the JSR authors. All rights reserved. MIT license.

import { useEffect, useRef, useState } from "preact/hooks";
import type { DownloadDataPoint } from "../../../utils/api_types.ts";
import {
  type AggregationPeriod,
  collectX,
  normalize,
} from "./DownloadChart.tsx";

interface Props {
  downloads: DownloadDataPoint[];
  scope: string;
  pkg: string;
}

const AGGREGATION_PERIOD: AggregationPeriod = "weekly";

export function DownloadWidget(props: Props) {
  const chartRef = useRef<HTMLDivElement>(null);

  const xValues = collectX(props.downloads, AGGREGATION_PERIOD);
  const data = normalize(props.downloads, xValues, AGGREGATION_PERIOD);

  let min = Infinity;
  let max = 0;
  for (const [_, number] of data) {
    if (number < min) {
      min = number;
    }
    if (number > max) {
      max = number;
    }
  }

  const [hoveredDataPoint, setHoveredDataPoint] = useState<
    { date: number; data: number } | null
  >(null);
  const [graphRendered, setGraphRendered] = useState(false);

  useEffect(() => {
    // deno-lint-ignore no-explicit-any
    let chart: any;
    (async () => {
      const { default: ApexCharts } = await import("apexcharts");
      chart = new ApexCharts(chartRef.current!, {
        chart: {
          type: "area",
          stacked: true,
          animations: {
            enabled: false,
          },
          height: "100%",
          width: "100%",
          zoom: {
            enabled: false,
          },
          toolbar: {
            show: false,
          },
          offsetX: 0,
          offsetY: 0,
          // sparkline mode strips all margins/axis by default
          sparkline: { enabled: true },
          events: {
            mouseLeave() {
              setHoveredDataPoint(null);
            },
          },
        },
        tooltip: {
          custom: ({ dataPointIndex }: { dataPointIndex: number }) => {
            const hoveredData = data[dataPointIndex];
            setHoveredDataPoint({
              date: hoveredData[0],
              data: hoveredData[1],
            });
            return "";
          },
        },
        legend: {
          show: false,
        },
        dataLabels: {
          enabled: false,
        },
        stroke: {
          curve: "straight",
          width: 2,
        },
        series: [{ data }],
        xaxis: {
          type: "datetime",
          labels: {
            show: false,
          },
          axisBorder: {
            show: false,
          },
          axisTicks: {
            show: false,
          },
        },
        yaxis: {
          labels: {
            show: false,
          },
          forceNiceScale: false,
          min: min * 0.5,
          max: max * 1.4,
        },
        grid: {
          show: false,
          padding: {
            left: 0,
            right: 0,
            top: 0,
            bottom: 0,
          },
        },
      });
      chart.render();
      setGraphRendered(true);
    })();
    return () => {
      chart.destroy();
    };
  }, []);

  return (
    <a
      className="flex flex-row gap-2"
      href={`/@${props.scope}/${props.pkg}/versions`}
    >
      <div
        class="font-mono text-xs space-y-2 z-10 text-nowrap"
        style={{ width: `${max.toString().length + 1}ch` }}
      >
        {graphRendered && (
          <>
            <div>
              {hoveredDataPoint
                ? `${
                  new Date(hoveredDataPoint.date).toISOString()
                    .split("T")[0]
                } to ${
                  new Date(
                    hoveredDataPoint.date + 6 * 24 * 60 * 60 * 1000,
                  ).toISOString()
                    .split("T")[0]
                }`
                : "Weekly downloads"}
            </div>
            <div>
              {hoveredDataPoint
                ? hoveredDataPoint.data.toLocaleString()
                : data.at(-1)![1].toLocaleString()}
            </div>
          </>
        )}
      </div>
      <div className="w-[150px] h-[50px]">
        <div ref={chartRef} class="minimal-chart" />
      </div>
    </a>
  );
}
