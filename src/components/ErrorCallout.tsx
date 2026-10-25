import { XCircle } from "lucide-react";
import { layerLabel } from "@/services/errors";
import { type ErrorLayer } from "@/types";

export interface CalloutError {
  message: string;
  hint?: string;
  layer?: ErrorLayer;
}

export function ErrorCallout({ error }: { error: CalloutError | null }) {
  if (!error) return null;
  const layer = layerLabel(error.layer);
  return (
    <div className="callout error selectable">
      <XCircle size={15} className="callout-icon" />
      <div>
        <div>
          {layer && <strong>{layer}: </strong>}
          {error.message}
        </div>
        {error.hint && <div className="hint">{error.hint}</div>}
      </div>
    </div>
  );
}

/** The callout with the page padding, for errors at the top of a tab. */
export function PaddedError({ error }: { error: CalloutError | null }) {
  if (!error) return null;
  return (
    <div className="pad">
      <ErrorCallout error={error} />
    </div>
  );
}
