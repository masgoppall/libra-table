// CON-8: a connection from a URL (typed, or opened from the OS as a deep link).

import { Link2 } from "lucide-react";
import { useState } from "react";
import { api } from "@/api";
import { ErrorCallout } from "@/components/ErrorCallout";
import { Modal } from "@/components/Modal";
import { ModalActions } from "@/components/ModalActions";
import { type AppError } from "@/types";
import { type FormInitial } from "./form/useConnectionForm";

const EXAMPLE = "postgresql://user:password@host:5432/database";

export function ImportUrlDialog(props: {
  initial: string;
  onClose: () => void;
  onDraft: (f: FormInitial) => void;
}) {
  const [url, setUrl] = useState(props.initial);
  const [error, setError] = useState<AppError | null>(null);
  const go = async () => {
    try {
      props.onDraft(await api.parseConnectionUrl(url));
    } catch (e) {
      setError(e as AppError);
    }
  };
  return (
    <Modal
      title="Import from URL"
      icon={<Link2 size={18} />}
      onClose={props.onClose}
      footer={
        <ModalActions
          onCancel={props.onClose}
          onOk={go}
          okLabel="Continue"
          disabled={!url.trim()}
        />
      }
    >
      <input
        className="input mono"
        autoFocus
        placeholder={EXAMPLE}
        value={url}
        onChange={(e) => setUrl(e.target.value)}
        onKeyDown={(e) => e.key === "Enter" && go()}
      />
      <div className="faint small">
        Supports postgresql://, postgres://, mysql://, sqlite:/// and
        postgresql+ssh://ssh_user@ssh_host/db_user@db_host/database. Passwords in the URL go to the
        Keychain when you save.
      </div>
      <ErrorCallout error={error} />
    </Modal>
  );
}
