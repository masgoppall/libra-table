import { CheckCircle2, Info, X, XCircle } from "lucide-react";
import { type Toast, type ToastKind, useStore } from "@/store";

const ICONS: Record<ToastKind, React.ReactNode> = {
  error: <XCircle size={16} color="var(--danger)" />,
  success: <CheckCircle2 size={16} color="var(--ok)" />,
  info: <Info size={16} color="var(--accent)" />,
};

function ToastItem({ toast, onDismiss }: { toast: Toast; onDismiss: () => void }) {
  return (
    <div className={`toast ${toast.kind}`}>
      {ICONS[toast.kind]}
      <div className="toast-body selectable">
        <div className="toast-message">{toast.message}</div>
        {toast.detail && <div className="detail-text">{toast.detail}</div>}
      </div>
      <button className="btn ghost icon xs" onClick={onDismiss} aria-label="Dismiss">
        <X size={12} />
      </button>
    </div>
  );
}

export function Toasts() {
  const toasts = useStore((s) => s.toasts);
  const dismiss = useStore((s) => s.dismissToast);
  return (
    <div className="toasts" aria-live="polite">
      {toasts.map((t) => (
        <ToastItem key={t.id} toast={t} onDismiss={() => dismiss(t.id)} />
      ))}
    </div>
  );
}
