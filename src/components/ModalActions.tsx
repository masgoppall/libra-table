// The usual dialog footer: something on the left, then Cancel and the main button.

import { type ReactNode } from "react";
import { Spinner } from "./Spinner";

export interface ModalActionsProps {
  onCancel: () => void;
  onOk: () => void;
  okLabel: ReactNode;
  cancelLabel?: string;
  danger?: boolean;
  disabled?: boolean;
  busy?: boolean;
  /** Focus Cancel, so Enter doesn't run a destructive action. */
  focusCancel?: boolean;
  leading?: ReactNode;
}

export function ModalActions(props: ModalActionsProps) {
  return (
    <>
      {props.leading}
      <span className="spacer" />
      <button className="btn" autoFocus={props.focusCancel} onClick={props.onCancel}>
        {props.cancelLabel ?? "Cancel"}
      </button>
      <button
        className={`btn ${props.danger ? "danger" : "primary"}`}
        disabled={props.disabled || props.busy}
        onClick={props.onOk}
      >
        {props.busy && <Spinner />} {props.okLabel}
      </button>
    </>
  );
}
