import {
  Bar,
  BarChart,
  CartesianGrid,
  Cell,
  Line,
  LineChart,
  Pie,
  PieChart,
  ResponsiveContainer,
  Tooltip,
  XAxis,
  YAxis,
} from "recharts";
import {
  chartColor,
  chartTooltipLabelStyle,
  chartTooltipStyle,
} from "../../app/constants";
import { useI18n } from "../../shared/i18n";

export type ChartDatum = {
  displayValue?: string;
  id: string;
  label: string;
  value: number;
};

function isObjectRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function formatCompactNumber(value: number) {
  return new Intl.NumberFormat("en", {
    maximumFractionDigits: 1,
    notation: "compact",
  }).format(value);
}

function chartPayloadLabel(payload: unknown) {
  return isObjectRecord(payload) && typeof payload.label === "string"
    ? payload.label
    : "";
}

function chartPayloadDisplayValue(
  payload: unknown,
  valueFormatter: (value: number) => string,
  fallbackValue?: unknown,
) {
  if (isObjectRecord(payload) && typeof payload.displayValue === "string") {
    return payload.displayValue;
  }

  if (isObjectRecord(payload) && typeof payload.value === "number") {
    return valueFormatter(payload.value);
  }

  const numberValue = Number(fallbackValue);
  return Number.isFinite(numberValue) ? valueFormatter(numberValue) : "n/a";
}

function compactChartTick(value: unknown) {
  const numberValue = Number(value);
  if (!Number.isFinite(numberValue)) {
    return String(value);
  }

  return formatCompactNumber(numberValue);
}

function compactChartLabel(value: unknown) {
  const label = String(value);
  return label.length > 16 ? `${label.slice(0, 15)}...` : label;
}

export function LineChartCard({
  data,
  title,
  valueFormatter,
}: {
  data: ChartDatum[];
  title: string;
  valueFormatter: (value: number) => string;
}) {
  const { t } = useI18n();
  const chartData = data.slice(-12);

  return (
    <section className="rounded-2xl border border-stone-200 bg-white/85 px-4 py-4 shadow-[0_18px_42px_rgba(75,63,42,0.07)]">
      <h3 className="text-sm font-semibold text-stone-950">{title}</h3>
      {chartData.length ? (
        <div className="mt-3 h-52 w-full">
          <ResponsiveContainer
            height="100%"
            initialDimension={{ height: 208, width: 720 }}
            width="100%"
          >
            <LineChart
              data={chartData}
              margin={{ bottom: 4, left: 0, right: 12, top: 10 }}
            >
              <CartesianGrid stroke="#f5f5f4" vertical={false} />
              <XAxis
                axisLine={false}
                dataKey="label"
                minTickGap={18}
                tick={{ fill: "#78716c", fontSize: 12 }}
                tickLine={false}
              />
              <YAxis
                axisLine={false}
                tick={{ fill: "#78716c", fontSize: 12 }}
                tickFormatter={compactChartTick}
                tickLine={false}
                width={46}
              />
              <Tooltip
                contentStyle={chartTooltipStyle}
                cursor={{ stroke: "#99f6e4", strokeWidth: 1 }}
                formatter={(value) => [valueFormatter(Number(value)), title]}
                labelStyle={chartTooltipLabelStyle}
              />
              <Line
                activeDot={{ r: 6, stroke: "#0f766e", strokeWidth: 2 }}
                dataKey="value"
                dot={{
                  fill: "#ffffff",
                  r: 3,
                  stroke: "#0f766e",
                  strokeWidth: 2,
                }}
                isAnimationActive
                name={title}
                stroke="#0f766e"
                strokeWidth={2.5}
                type="monotone"
              />
            </LineChart>
          </ResponsiveContainer>
        </div>
      ) : (
        <ChartEmptyState label={t("No chart data")} />
      )}
    </section>
  );
}

export function DonutChartCard({
  data,
  title,
  valueFormatter,
}: {
  data: ChartDatum[];
  title: string;
  valueFormatter: (value: number) => string;
}) {
  const { t } = useI18n();
  const chartData = data.filter((item) => item.value > 0).slice(0, 6);
  const total = chartData.reduce((sum, item) => sum + item.value, 0);

  return (
    <section className="rounded-2xl border border-stone-200 bg-white/85 px-4 py-4 shadow-[0_18px_42px_rgba(75,63,42,0.07)]">
      <h3 className="text-sm font-semibold text-stone-950">{title}</h3>
      {total > 0 ? (
        <div className="mt-4 grid gap-4 sm:grid-cols-[12rem_1fr] sm:items-center">
          <div className="relative h-48 w-full min-w-0">
            <ResponsiveContainer
              height="100%"
              initialDimension={{ height: 192, width: 192 }}
              width="100%"
            >
              <PieChart>
                <Tooltip
                  contentStyle={chartTooltipStyle}
                  formatter={(value, _name, item) => [
                    valueFormatter(Number(value)),
                    chartPayloadLabel(item.payload),
                  ]}
                  labelStyle={chartTooltipLabelStyle}
                />
                <Pie
                  animationDuration={450}
                  data={chartData}
                  dataKey="value"
                  innerRadius="58%"
                  nameKey="label"
                  outerRadius="82%"
                  paddingAngle={2}
                >
                  {chartData.map((item, index) => (
                    <Cell fill={chartColor(index)} key={item.id} />
                  ))}
                </Pie>
              </PieChart>
            </ResponsiveContainer>
            <div className="pointer-events-none absolute inset-0 grid place-items-center">
              <div className="rounded-full bg-white/80 px-2 py-1 text-center font-mono text-sm font-semibold text-stone-950 shadow-sm">
                {valueFormatter(total)}
              </div>
            </div>
          </div>
          <ChartLegend data={chartData} valueFormatter={valueFormatter} />
        </div>
      ) : (
        <ChartEmptyState label={t("No chart data")} />
      )}
    </section>
  );
}

export function BarChartCard({
  data,
  maxValue,
  title,
  valueFormatter,
}: {
  data: ChartDatum[];
  maxValue?: number;
  title: string;
  valueFormatter: (value: number) => string;
}) {
  const { t } = useI18n();
  const chartData = data.filter((item) => item.value > 0).slice(0, 8);
  const chartMax = Math.max(maxValue ?? 0, ...chartData.map((item) => item.value), 1);

  return (
    <section className="rounded-2xl border border-stone-200 bg-white/85 px-4 py-4 shadow-[0_18px_42px_rgba(75,63,42,0.07)]">
      <h3 className="text-sm font-semibold text-stone-950">{title}</h3>
      {chartData.length ? (
        <>
          <div className="mt-4 h-64 w-full">
            <ResponsiveContainer
              height="100%"
              initialDimension={{ height: 256, width: 720 }}
              width="100%"
            >
              <BarChart
                data={chartData}
                layout="vertical"
                margin={{ bottom: 4, left: 6, right: 18, top: 4 }}
              >
                <CartesianGrid horizontal={false} stroke="#f5f5f4" />
                <XAxis domain={[0, chartMax]} hide type="number" />
                <YAxis
                  axisLine={false}
                  dataKey="label"
                  tick={{ fill: "#78716c", fontSize: 12 }}
                  tickFormatter={compactChartLabel}
                  tickLine={false}
                  type="category"
                  width={112}
                />
                <Tooltip
                  contentStyle={chartTooltipStyle}
                  cursor={{ fill: "#f0fdfa" }}
                  formatter={(value, _name, item) => [
                    chartPayloadDisplayValue(
                      item.payload,
                      valueFormatter,
                      value,
                    ),
                    chartPayloadLabel(item.payload),
                  ]}
                  labelStyle={chartTooltipLabelStyle}
                />
                <Bar
                  animationDuration={450}
                  barSize={16}
                  dataKey="value"
                  radius={[0, 8, 8, 0]}
                >
                  {chartData.map((item, index) => (
                    <Cell fill={chartColor(index)} key={item.id} />
                  ))}
                </Bar>
              </BarChart>
            </ResponsiveContainer>
          </div>
          <ChartLegend data={chartData} valueFormatter={valueFormatter} />
        </>
      ) : (
        <ChartEmptyState label={t("No chart data")} />
      )}
    </section>
  );
}

function ChartLegend({
  data,
  valueFormatter,
}: {
  data: ChartDatum[];
  valueFormatter: (value: number) => string;
}) {
  return (
    <div className="grid gap-2">
      {data.map((item, index) => (
        <div className="flex min-w-0 items-center gap-2 text-xs" key={item.id}>
          <span
            aria-hidden="true"
            className="size-2.5 shrink-0 rounded-full"
            style={{ backgroundColor: chartColor(index) }}
          />
          <span className="min-w-0 flex-1 truncate font-medium text-stone-600">
            {item.label}
          </span>
          <span className="shrink-0 font-mono text-stone-950">
            {item.displayValue ?? valueFormatter(item.value)}
          </span>
        </div>
      ))}
    </div>
  );
}

function ChartEmptyState({ label }: { label: string }) {
  return (
    <div className="mt-4 grid h-44 place-items-center rounded-xl border border-dashed border-stone-300 bg-stone-50/70 text-sm font-medium text-stone-500">
      {label}
    </div>
  );
}
