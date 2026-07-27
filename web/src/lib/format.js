export function formatMetric(metric, value, compact = false) {
  if (metric === "tokens") return `${(value / 1000).toFixed(compact ? 0 : 1)}k`;
  if (metric === "duration") return `${value.toFixed(compact ? 0 : 1)}s`;
  return `${value.toFixed(value % 1 === 0 ? 0 : 1)}${metric === "successRate" ? "%" : ""}`;
}

export function metricRange(row, metric, definitions) {
  const definition = definitions[metric];
  if (!definition.minKey || !definition.maxKey) return null;
  return [row[definition.minKey], row[definition.maxKey]];
}

export function sentenceCase(value) {
  return value.charAt(0).toUpperCase() + value.slice(1);
}

export function chartDomain(
  rows,
  metric,
  definitions,
  { bounds = [0, null], includeRanges = false } = {},
) {
  const definition = definitions[metric];
  const values = rows.flatMap((row) => {
    const rowValues = [row[definition.key]];
    if (includeRanges && definition.minKey) rowValues.push(row[definition.minKey]);
    if (includeRanges && definition.maxKey) rowValues.push(row[definition.maxKey]);
    return rowValues.filter(Number.isFinite);
  });

  if (values.length === 0) {
    return [
      bounds[0] ?? 0,
      bounds[1] ?? 1,
    ];
  }

  const min = Math.min(...values);
  const max = Math.max(...values);
  const boundedSpan = bounds[0] !== null && bounds[1] !== null
    ? (bounds[1] - bounds[0]) * 0.08
    : 0;
  const minimumSpan = Math.max(boundedSpan, Math.abs(max) * 0.1, 1);
  const span = Math.max(max - min, minimumSpan);
  const center = (min + max) / 2;
  const padding = span * 0.1;
  const domainMin = center - span / 2 - padding;
  const domainMax = center + span / 2 + padding;

  return [
    bounds[0] === null ? domainMin : Math.max(bounds[0], domainMin),
    bounds[1] === null ? domainMax : Math.min(bounds[1], domainMax),
  ];
}
