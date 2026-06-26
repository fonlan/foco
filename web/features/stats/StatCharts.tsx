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
  Scatter,
  ScatterChart,
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

export type DualChartDatum = {
  id: string;
  label: string;
  primaryValue: number;
  secondaryValue: number;
};

export type ScatterMetricDatum = {
  displayXValue?: string;
  displayYValue?: string;
  id: string;
  label: string;
  x: number;
  y: number;
};

type RingDatum = ChartDatum & {
  metricLabel: string;
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

function chartPayloadMetricLabel(payload: unknown) {
  return isObjectRecord(payload) && typeof payload.metricLabel === "string"
    ? payload.metricLabel
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

function chartPayloadDisplayMetricValue(
  payload: unknown,
  key: "displayXValue" | "displayYValue",
  valueFormatter: (value: number) => string,
  fallbackValue?: unknown,
) {
  if (isObjectRecord(payload) && typeof payload[key] === "string") {
    return payload[key];
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

export function DualLineChartCard({
  data,
  primaryFormatter,
  primaryLabel,
  secondaryFormatter,
  secondaryLabel,
  title,
}: {
  data: DualChartDatum[];
  primaryFormatter: (value: number) => string;
  primaryLabel: string;
  secondaryFormatter: (value: number) => string;
  secondaryLabel: string;
  title: string;
}) {
  const { t } = useI18n();
  const chartData = data.slice(-12);

  return (
    <section className="rounded-2xl border border-stone-200 bg-white/85 px-4 py-4 shadow-[0_18px_42px_rgba(75,63,42,0.07)]">
      <h3 className="text-sm font-semibold text-stone-950">{title}</h3>
      {chartData.length ? (
        <>
          <div className="mt-3 h-52 w-full">
            <ResponsiveContainer
              height="100%"
              initialDimension={{ height: 208, width: 720 }}
              width="100%"
            >
              <LineChart
                data={chartData}
                margin={{ bottom: 4, left: 0, right: 8, top: 10 }}
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
                  yAxisId="requests"
                />
                <YAxis
                  axisLine={false}
                  orientation="right"
                  tick={{ fill: "#78716c", fontSize: 12 }}
                  tickFormatter={compactChartTick}
                  tickLine={false}
                  width={48}
                  yAxisId="tokens"
                />
                <Tooltip
                  contentStyle={chartTooltipStyle}
                  cursor={{ stroke: "#99f6e4", strokeWidth: 1 }}
                  formatter={(value, name) => {
                    const isSecondary = String(name) === secondaryLabel;
                    return [
                      isSecondary
                        ? secondaryFormatter(Number(value))
                        : primaryFormatter(Number(value)),
                      isSecondary ? secondaryLabel : primaryLabel,
                    ];
                  }}
                  labelStyle={chartTooltipLabelStyle}
                />
                <Line
                  activeDot={{ r: 6, stroke: "#0f766e", strokeWidth: 2 }}
                  dataKey="primaryValue"
                  dot={{ fill: "#ffffff", r: 3, stroke: "#0f766e", strokeWidth: 2 }}
                  isAnimationActive
                  name={primaryLabel}
                  stroke="#0f766e"
                  strokeWidth={2.5}
                  type="monotone"
                  yAxisId="requests"
                />
                <Line
                  activeDot={{ r: 6, stroke: "#b45309", strokeWidth: 2 }}
                  dataKey="secondaryValue"
                  dot={{ fill: "#ffffff", r: 3, stroke: "#b45309", strokeWidth: 2 }}
                  isAnimationActive
                  name={secondaryLabel}
                  stroke="#b45309"
                  strokeWidth={2.5}
                  type="monotone"
                  yAxisId="tokens"
                />
              </LineChart>
            </ResponsiveContainer>
          </div>
          <SeriesLegend
            items={[
              { color: "#0f766e", label: primaryLabel },
              { color: "#b45309", label: secondaryLabel },
            ]}
          />
        </>
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

export function DoubleDonutChartCard({
  innerData,
  innerFormatter,
  innerLabel,
  outerData,
  outerFormatter,
  outerLabel,
  title,
}: {
  innerData: ChartDatum[];
  innerFormatter: (value: number) => string;
  innerLabel: string;
  outerData: ChartDatum[];
  outerFormatter: (value: number) => string;
  outerLabel: string;
  title: string;
}) {
  const { t } = useI18n();
  const outerChartData: RingDatum[] = outerData
    .filter((item) => item.value > 0)
    .slice(0, 6)
    .map((item) => ({ ...item, metricLabel: outerLabel }));
  const innerChartData: RingDatum[] = innerData
    .filter((item) => item.value > 0)
    .slice(0, 6)
    .map((item) => ({ ...item, metricLabel: innerLabel }));
  const outerTotal = outerChartData.reduce((sum, item) => sum + item.value, 0);
  const innerTotal = innerChartData.reduce((sum, item) => sum + item.value, 0);
  const colorIds = Array.from(
    new Set([...outerChartData, ...innerChartData].map((item) => item.id)),
  );
  const colorForItem = (item: ChartDatum) => chartColor(colorIds.indexOf(item.id));

  return (
    <section className="rounded-2xl border border-stone-200 bg-white/85 px-4 py-4 shadow-[0_18px_42px_rgba(75,63,42,0.07)]">
      <h3 className="text-sm font-semibold text-stone-950">{title}</h3>
      {outerTotal > 0 || innerTotal > 0 ? (
        <div className="mt-4 grid gap-4 sm:grid-cols-[12rem_1fr] sm:items-center">
          <div className="relative h-48 w-full min-w-0">
            <div className="relative z-10 h-full w-full">
              <ResponsiveContainer
                height="100%"
                initialDimension={{ height: 192, width: 192 }}
                width="100%"
              >
                <PieChart>
                  <Tooltip
                    contentStyle={chartTooltipStyle}
                    formatter={(value, _name, item) => {
                      const metricLabel = chartPayloadMetricLabel(item.payload);
                      const formatter =
                        metricLabel === innerLabel ? innerFormatter : outerFormatter;
                      return [
                        formatter(Number(value)),
                        `${chartPayloadLabel(item.payload)} · ${metricLabel}`,
                      ];
                    }}
                    labelStyle={chartTooltipLabelStyle}
                  />
                  <Pie
                    animationDuration={450}
                    data={outerChartData}
                    dataKey="value"
                    innerRadius="70%"
                    nameKey="label"
                    outerRadius="88%"
                    paddingAngle={2}
                  >
                    {outerChartData.map((item) => (
                      <Cell fill={colorForItem(item)} key={`outer-${item.id}`} />
                    ))}
                  </Pie>
                  <Pie
                    animationDuration={450}
                    data={innerChartData}
                    dataKey="value"
                    innerRadius="42%"
                    nameKey="label"
                    outerRadius="60%"
                    paddingAngle={2}
                  >
                    {innerChartData.map((item) => (
                      <Cell fill={colorForItem(item)} key={`inner-${item.id}`} />
                    ))}
                  </Pie>
                </PieChart>
              </ResponsiveContainer>
            </div>
            <div className="pointer-events-none absolute inset-0 z-0 grid place-items-center text-center">
              <div className="rounded-full bg-white/85 px-2 py-1 shadow-sm">
                <div className="font-mono text-xs font-semibold text-stone-950">
                  {outerFormatter(outerTotal)}
                </div>
                <div className="font-mono text-[11px] font-semibold text-stone-500">
                  {innerFormatter(innerTotal)}
                </div>
              </div>
            </div>
          </div>
          <DualChartLegend
            innerData={innerChartData}
            innerFormatter={innerFormatter}
            innerLabel={innerLabel}
            outerData={outerChartData}
            outerFormatter={outerFormatter}
            outerLabel={outerLabel}
          />
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

export function ScatterChartCard({
  data,
  title,
  xFormatter,
  xLabel,
  yFormatter,
  yLabel,
}: {
  data: ScatterMetricDatum[];
  title: string;
  xFormatter: (value: number) => string;
  xLabel: string;
  yFormatter: (value: number) => string;
  yLabel: string;
}) {
  const { t } = useI18n();
  const chartData = data
    .filter((item) => Number.isFinite(item.x) && Number.isFinite(item.y))
    .slice(0, 12);
  const xMax = Math.max(...chartData.map((item) => item.x), 1);

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
              <ScatterChart margin={{ bottom: 8, left: 0, right: 16, top: 8 }}>
                <CartesianGrid stroke="#f5f5f4" />
                <XAxis
                  axisLine={false}
                  dataKey="x"
                  domain={[0, xMax]}
                  name={xLabel}
                  tick={{ fill: "#78716c", fontSize: 12 }}
                  tickFormatter={(value) => xFormatter(Number(value))}
                  tickLine={false}
                  type="number"
                />
                <YAxis
                  axisLine={false}
                  dataKey="y"
                  domain={[0, 1]}
                  name={yLabel}
                  tick={{ fill: "#78716c", fontSize: 12 }}
                  tickFormatter={(value) => yFormatter(Number(value))}
                  tickLine={false}
                  type="number"
                  width={46}
                />
                <Tooltip
                  contentStyle={chartTooltipStyle}
                  cursor={{ stroke: "#99f6e4", strokeWidth: 1 }}
                  formatter={(value, name, item) => {
                    const isXValue = String(name) === xLabel;
                    return [
                      chartPayloadDisplayMetricValue(
                        item.payload,
                        isXValue ? "displayXValue" : "displayYValue",
                        isXValue ? xFormatter : yFormatter,
                        value,
                      ),
                      isXValue ? xLabel : yLabel,
                    ];
                  }}
                  labelFormatter={(_label, payload) =>
                    payload[0]?.payload ? chartPayloadLabel(payload[0].payload) : ""
                  }
                  labelStyle={chartTooltipLabelStyle}
                />
                <Scatter data={chartData} fill="#0f766e" name={title}>
                  {chartData.map((item, index) => (
                    <Cell fill={chartColor(index)} key={item.id} />
                  ))}
                </Scatter>
              </ScatterChart>
            </ResponsiveContainer>
          </div>
          <ScatterLegend
            data={chartData}
            xFormatter={xFormatter}
            xLabel={xLabel}
            yFormatter={yFormatter}
            yLabel={yLabel}
          />
        </>
      ) : (
        <ChartEmptyState label={t("No chart data")} />
      )}
    </section>
  );
}

function SeriesLegend({
  items,
}: {
  items: Array<{ color: string; label: string }>;
}) {
  return (
    <div className="mt-3 flex flex-wrap gap-3 text-xs font-medium text-stone-600">
      {items.map((item) => (
        <span className="inline-flex items-center gap-1.5" key={item.label}>
          <span
            aria-hidden="true"
            className="size-2.5 rounded-full"
            style={{ backgroundColor: item.color }}
          />
          {item.label}
        </span>
      ))}
    </div>
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

function DualChartLegend({
  innerData,
  innerFormatter,
  innerLabel,
  outerData,
  outerFormatter,
  outerLabel,
}: {
  innerData: ChartDatum[];
  innerFormatter: (value: number) => string;
  innerLabel: string;
  outerData: ChartDatum[];
  outerFormatter: (value: number) => string;
  outerLabel: string;
}) {
  const rows = Array.from(
    new Map([...outerData, ...innerData].map((item) => [item.id, item])).values(),
  );

  return (
    <div className="grid gap-2">
      <div className="grid grid-cols-[1fr_auto_auto] gap-3 text-[11px] font-semibold uppercase text-stone-400">
        <span />
        <span>{outerLabel}</span>
        <span>{innerLabel}</span>
      </div>
      {rows.map((item, index) => {
        const outerItem = outerData.find((candidate) => candidate.id === item.id);
        const innerItem = innerData.find((candidate) => candidate.id === item.id);
        return (
          <div
            className="grid min-w-0 grid-cols-[1fr_auto_auto] items-center gap-3 text-xs"
            key={item.id}
          >
            <span className="flex min-w-0 items-center gap-2">
              <span
                aria-hidden="true"
                className="size-2.5 shrink-0 rounded-full"
                style={{ backgroundColor: chartColor(index) }}
              />
              <span className="min-w-0 truncate font-medium text-stone-600">
                {item.label}
              </span>
            </span>
            <span className="font-mono text-stone-950">
              {outerItem ? outerFormatter(outerItem.value) : "n/a"}
            </span>
            <span className="font-mono text-stone-950">
              {innerItem ? innerFormatter(innerItem.value) : "n/a"}
            </span>
          </div>
        );
      })}
    </div>
  );
}

function ScatterLegend({
  data,
  xFormatter,
  xLabel,
  yFormatter,
  yLabel,
}: {
  data: ScatterMetricDatum[];
  xFormatter: (value: number) => string;
  xLabel: string;
  yFormatter: (value: number) => string;
  yLabel: string;
}) {
  return (
    <div className="mt-2 grid gap-2">
      <div className="grid grid-cols-[1fr_auto_auto] gap-3 text-[11px] font-semibold uppercase text-stone-400">
        <span />
        <span>{xLabel}</span>
        <span>{yLabel}</span>
      </div>
      {data.map((item, index) => (
        <div
          className="grid min-w-0 grid-cols-[1fr_auto_auto] items-center gap-3 text-xs"
          key={item.id}
        >
          <span className="flex min-w-0 items-center gap-2">
            <span
              aria-hidden="true"
              className="size-2.5 shrink-0 rounded-full"
              style={{ backgroundColor: chartColor(index) }}
            />
            <span className="min-w-0 truncate font-medium text-stone-600">
              {item.label}
            </span>
          </span>
          <span className="font-mono text-stone-950">
            {item.displayXValue ?? xFormatter(item.x)}
          </span>
          <span className="font-mono text-stone-950">
            {item.displayYValue ?? yFormatter(item.y)}
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
