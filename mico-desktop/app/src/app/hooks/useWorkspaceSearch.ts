import { useEffect, useState } from "react";

import type { WorkspaceSearchResult } from "../../types";

export function useWorkspaceSearch(query: string) {
  const [results, setResults] = useState<WorkspaceSearchResult[]>([]);
  const [highlightedIndex, setHighlightedIndex] = useState(0);

  useEffect(() => {
    if (!query.trim()) {
      setResults([]);
      setHighlightedIndex(0);
      return;
    }

    let cancelled = false;
    const timer = window.setTimeout(() => {
      void window.mico
        .searchWorkspace(query, 8)
        .then((next) => {
          if (!cancelled) {
            setResults(Array.isArray(next) ? next : []);
          }
        })
        .catch(() => {
          if (!cancelled) {
            setResults([]);
          }
        });
    }, 100);

    return () => {
      cancelled = true;
      window.clearTimeout(timer);
    };
  }, [query]);

  useEffect(() => {
    if (highlightedIndex >= results.length) {
      setHighlightedIndex(0);
    }
  }, [highlightedIndex, results.length]);

  return {
    highlightedIndex,
    results,
    setHighlightedIndex,
  };
}
