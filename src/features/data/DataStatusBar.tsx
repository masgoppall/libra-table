// Row range and count, timing, the page's SQL, page size and the pager.

import { ChevronLeft, ChevronRight, Code2 } from "lucide-react";
import { withShortcut } from "@/lib/actions";
import { formatCount, formatDuration } from "@/lib/format";
import { type PageResult, type RowCount } from "@/types";

export const PAGE_SIZES = [100, 300, 500, 1000, 2000, 5000];

export interface DataStatusBarProps {
  result: PageResult | null;
  count: RowCount | null;
  page: number;
  pageSize: number;
  loading: boolean;
  selected: number;
  showSql: boolean;
  onToggleSql: () => void;
  onPage: (page: number) => void;
  onPageSize: (size: number) => void;
}

const approx = (count: RowCount | null) => (count?.exact ? "" : "~");

function RowRange(p: DataStatusBarProps) {
  const rows = p.result?.rows.length ?? 0;
  const first = p.page * p.pageSize + 1;
  const last = p.page * p.pageSize + rows;
  return (
    <span className="status-strong">
      {rows > 0 ? `${formatCount(first)}–${formatCount(last)}` : "No rows"}
      {p.count && (
        <span className="muted">
          {" "}
          of {approx(p.count)}
          {formatCount(p.count.total)}
        </span>
      )}
    </span>
  );
}

function Pager(p: DataStatusBarProps & { totalPages: number | null }) {
  const go = (value: string) => {
    const n = Number(value);
    if (n >= 1 && (!p.totalPages || n <= p.totalPages)) p.onPage(n - 1);
  };
  return (
    <div className="pager">
      <button
        className="btn xs icon"
        disabled={p.page === 0 || p.loading}
        onClick={() => p.onPage(p.page - 1)}
        aria-label="Previous page"
        title={withShortcut("Previous page", "prev-page")}
      >
        <ChevronLeft size={13} />
      </button>
      <span className="pager-text">
        <input className="pager-input" value={p.page + 1} onChange={(e) => go(e.target.value)} />
        {p.totalPages && (
          <span className="faint">
            {" "}
            / {approx(p.count)}
            {formatCount(p.totalPages)}
          </span>
        )}
      </span>
      <button
        className="btn xs icon"
        disabled={!p.result?.hasMore || p.loading}
        onClick={() => p.onPage(p.page + 1)}
        aria-label="Next page"
        title={withShortcut("Next page", "next-page")}
      >
        <ChevronRight size={13} />
      </button>
    </div>
  );
}

export function DataStatusBar(p: DataStatusBarProps) {
  const totalPages = p.count ? Math.max(1, Math.ceil(p.count.total / p.pageSize)) : null;
  const sizes = [...new Set([...PAGE_SIZES, p.pageSize])].sort((a, b) => a - b);
  return (
    <div className="statusbar">
      <RowRange {...p} />
      {p.result && <span className="faint">{formatDuration(p.result.elapsedMs)}</span>}
      {p.selected > 0 && <span className="status-pill">{p.selected} selected</span>}
      <button
        className={`btn xs ghost${p.showSql ? " active" : ""}`}
        onClick={p.onToggleSql}
        title="Show the SELECT that loaded this page"
      >
        <Code2 size={12} /> SQL
      </button>
      <span className="spacer" />
      <select
        className="select xs"
        value={p.pageSize}
        onChange={(e) => p.onPageSize(Number(e.target.value))}
        title="Rows per page"
      >
        {sizes.map((n) => (
          <option key={n} value={n}>
            {n} / page
          </option>
        ))}
      </select>
      <Pager {...p} totalPages={totalPages} />
    </div>
  );
}
