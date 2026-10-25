// Import wizard (ImportCSVWizard / ImportJSONWizard): file → columns → import.

import { FileUp } from "lucide-react";
import { useEffect } from "react";
import { Modal } from "@/components/Modal";
import { type ToolDialog } from "@/store";
import { type Dialect } from "@/types";
import { ColumnsStep } from "./ColumnsStep";
import { RunStep } from "./RunStep";
import { SourceStep } from "./SourceStep";
import { type ImportStep, type ImportWizardState, useImportWizard } from "./useImportWizard";

const STEPS: [ImportStep, string][] = [
  ["source", "File"],
  ["columns", "Columns"],
  ["run", "Import"],
];

function Steps({ step }: { step: ImportStep }) {
  return (
    <span className="import-steps">
      {STEPS.map(([s, label], i) => (
        <span key={s} className={s === step ? "current" : undefined}>
          {i + 1} {label}
        </span>
      ))}
    </span>
  );
}

function Footer({ w, onClose }: { w: ImportWizardState; onClose: () => void }) {
  const { step, job } = w;
  if (step === "run") {
    return job.running ? (
      <>
        <span className="spacer" />
        <button className="btn" onClick={job.cancel}>
          Cancel import
        </button>
      </>
    ) : (
      <>
        <span className="spacer" />
        <button className="btn" onClick={() => (job.reset(), w.setStep("columns"))}>
          Back
        </button>
        <button className="btn primary" onClick={onClose}>
          Done
        </button>
      </>
    );
  }
  const next = step === "source";
  return (
    <>
      {!next && (
        <button className="btn" onClick={() => w.setStep("source")}>
          Back
        </button>
      )}
      <span className="spacer" />
      <button className="btn" onClick={onClose}>
        Cancel
      </button>
      {next ? (
        <button className="btn primary" disabled={!w.preview} onClick={() => w.setStep("columns")}>
          Next
        </button>
      ) : (
        <button className="btn primary" disabled={!!w.problem} onClick={w.start}>
          Import
        </button>
      )}
    </>
  );
}

export function ImportWizard(props: {
  dialog: Extract<ToolDialog, { kind: "import" }>;
  driver: Dialect;
  onClose: () => void;
}) {
  const w = useImportWizard(props.dialog, props.driver);
  const { step, job } = w;
  // Declining the confirmation leaves nothing to show: go back to the columns.
  const idle = step === "run" && !job.running && !job.summary && !job.error;
  useEffect(() => {
    if (idle) w.setStep("columns");
  }, [idle]);
  const table = props.dialog.table;
  return (
    <Modal
      title={table ? `Import into ${table}` : "Import into a new table"}
      subtitle={<Steps step={step} />}
      icon={<FileUp size={16} />}
      size="lg"
      onClose={() => !job.running && props.onClose()}
      footer={<Footer w={w} onClose={props.onClose} />}
    >
      {step === "source" && <SourceStep w={w} />}
      {step === "columns" && <ColumnsStep w={w} driver={props.driver} />}
      {step === "run" && <RunStep job={job} />}
    </Modal>
  );
}
