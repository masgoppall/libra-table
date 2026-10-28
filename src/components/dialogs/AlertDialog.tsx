// A message that only needs acknowledging: what is wrong (or fine) and an OK button.

import { AlertTriangle, CheckCircle2, XCircle } from "lucide-react";
import { type DialogResult, type DialogSpec } from "@/store";
import { BrandArt } from "../BrandArt";
import { useDialogLayer } from "../Modal";

type AlertSpec = Extract<DialogSpec, { kind: "alert" }>;

const ICON = {
  error: <XCircle size={30} strokeWidth={1.8} />,
  warning: <AlertTriangle size={30} strokeWidth={1.8} />,
  success: <CheckCircle2 size={30} strokeWidth={1.8} />,
};

export function AlertDialog({ spec, done }: { spec: AlertSpec; done: (r: DialogResult) => void }) {
  const ok = () => done({ ok: true });
  const tone = spec.tone ?? "error";
  const icon = <div className="alert-icon">{ICON[tone]}</div>;
  useDialogLayer(ok);
  return (
    <div
      className="overlay alert-overlay"
      onMouseDown={(e) => e.target === e.currentTarget && ok()}
    >
      <div
        className={`alert-box ${tone}`}
        role="alertdialog"
        aria-modal="true"
        aria-labelledby="alert-title"
        aria-describedby="alert-message"
      >
        {spec.art ? <BrandArt name={spec.art} className="alert-art" fallback={icon} /> : icon}
        <div className="alert-title" id="alert-title">
          {spec.title}
        </div>
        <div className="alert-message selectable" id="alert-message">
          {spec.message}
        </div>
        {!!spec.details?.length && (
          <ul className="alert-details selectable">
            {spec.details.map((d) => (
              <li key={d}>{d}</li>
            ))}
          </ul>
        )}
        {spec.hint && <div className="alert-hint selectable">{spec.hint}</div>}
        <button className="btn primary alert-ok" autoFocus onClick={ok}>
          OK
        </button>
      </div>
    </div>
  );
}
