// Inputs shared by the form sections.

import { open } from "@tauri-apps/plugin-dialog";
import { FolderOpen } from "lucide-react";
import { cx } from "@/lib/cx";
import { type PasswordMode, type SecretChange } from "@/types";
import { type ConnectionFormState, typedSecret } from "./useConnectionForm";
import { type FieldKey } from "./validation";

const SAVED = "•••••••• saved in Keychain";

/** A form label: required fields get a mark, missing ones turn red after a save. */
export function FieldLabel(props: {
  form: ConnectionFormState;
  field: FieldKey;
  children: React.ReactNode;
}) {
  const { form, field } = props;
  const required = form.required.has(field);
  return (
    <label className={cx(required && "required", form.invalid(field) && "invalid") || undefined}>
      {props.children}
    </label>
  );
}

/** Attributes of the input of `field`: found by the "fill in" focus, red when missing. */
export function fieldAttrs(form: ConnectionFormState, field: FieldKey) {
  return {
    "data-field": field,
    "aria-required": form.required.has(field) || undefined,
    "aria-invalid": form.invalid(field) || undefined,
  };
}

function passwordPlaceholder(mode: PasswordMode, saved: boolean, change: SecretChange): string {
  if (mode === "ask") return "asked when connecting";
  if (mode === "none") return "no password";
  if (change.action === "clear") return "will be removed";
  return saved && change.action === "keep" ? SAVED : "password";
}

export function PasswordField(props: {
  label: string;
  mode: PasswordMode;
  onMode: (m: PasswordMode) => void;
  saved: boolean;
  change: SecretChange;
  onChange: (c: SecretChange) => void;
}) {
  const typed = props.change.action === "set" ? props.change.value : "";
  const canClear = props.saved && props.mode === "keychain" && props.change.action !== "clear";
  return (
    <>
      <label>{props.label}</label>
      <div className="row">
        <input
          className="input grow"
          type="password"
          autoComplete="new-password"
          disabled={props.mode !== "keychain"}
          placeholder={passwordPlaceholder(props.mode, props.saved, props.change)}
          value={typed}
          // An empty field keeps the saved password; "No password" is the way to have none.
          onChange={(e) => props.onChange(typedSecret(e.target.value))}
        />
        <select
          className="select"
          value={props.mode}
          onChange={(e) => props.onMode(e.target.value as PasswordMode)}
        >
          <option value="keychain">Store in Keychain</option>
          <option value="ask">Ask every time</option>
          <option value="none">No password</option>
        </select>
        {canClear && (
          <button
            className="btn sm ghost"
            onClick={() => props.onChange({ action: "clear" })}
            title="Remove the saved password"
          >
            Clear
          </button>
        )}
      </div>
    </>
  );
}

export function PathField(props: {
  value: string;
  onChange: (v: string) => void;
  placeholder?: string;
  title: string;
  attrs?: ReturnType<typeof fieldAttrs>;
}) {
  const browse = async () => {
    const p = await open({ multiple: false, directory: false, title: props.title });
    if (typeof p === "string") props.onChange(p);
  };
  return (
    <div className="row">
      <input
        className="input mono grow"
        {...props.attrs}
        value={props.value}
        placeholder={props.placeholder}
        onChange={(e) => props.onChange(e.target.value)}
      />
      <button className="btn icon" title={props.title} onClick={browse}>
        <FolderOpen size={14} />
      </button>
    </div>
  );
}

/** A note under a field, in the second column of a form grid. */
export function FieldNote({ children }: { children: React.ReactNode }) {
  return (
    <>
      <span />
      <div className="faint small">{children}</div>
    </>
  );
}
