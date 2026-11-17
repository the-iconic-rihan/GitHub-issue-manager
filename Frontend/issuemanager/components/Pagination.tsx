import type { PageMeta } from "@/lib/types";

export default function Pagination({
  meta,
  onPage,
  disabled,
}: {
  meta?: PageMeta;
  onPage: (page: number) => void;
  disabled?: boolean;
}) {
  if (!meta || (meta.page === 1 && !meta.has_next)) return null;
  const totalPages = meta.total_count !== undefined ? Math.max(1, Math.ceil(meta.total_count / meta.per_page)) : undefined;
  return (
    <nav className="mt-4 flex items-center justify-between gap-2" aria-label="Pagination">
      <button className="btn-secondary" disabled={disabled || meta.page <= 1} onClick={() => onPage(meta.page - 1)}>
        ← Previous
      </button>
      <span className="text-sm text-gray-600">
        Page {meta.page}
        {totalPages ? ` of ${totalPages}` : ""}
      </span>
      <button className="btn-secondary" disabled={disabled || !meta.has_next} onClick={() => onPage(meta.page + 1)}>
        Next →
      </button>
    </nav>
  );
}
