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

export function chartDomain(rows, metric, definitions, { fixedPercent = false } = {}) {
  if (fixedPercent) return [0, 100];
  const definition = definitions[metric];
  const values = rows.flatMap((row) => [
    row[definition.key],
    definition.minKey ? row[definition.minKey] : row[definition.key],
    definition.maxKey ? row[definition.maxKey] : row[definition.key],
  ]);
  const min = Math.min(...values);
  const max = Math.max(...values);
  const span = Math.max(max - min, max * 0.12, 1);
  return [Math.max(0, min - span * 0.12), max + span * 0.12];
}
