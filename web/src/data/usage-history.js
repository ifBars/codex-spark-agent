export function usageHistoryHasActivity(history) {
  return history?.availability === "available"
    && Number.isFinite(history.aggregate?.totalTokens?.total);
}
