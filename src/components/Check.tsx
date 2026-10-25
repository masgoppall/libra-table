import { type ReactNode } from "react";

export interface CheckProps {
  checked: boolean;
  onChange: (checked: boolean) => void;
  children?: ReactNode;
  disabled?: boolean;
  title?: string;
  className?: string;
}

/** A checkbox with its label. */
export function Check(props: CheckProps) {
  return (
    <label className={`check ${props.className ?? ""}`.trim()} title={props.title}>
      <input
        type="checkbox"
        checked={props.checked}
        disabled={props.disabled}
        onChange={(e) => props.onChange(e.target.checked)}
      />
      {props.children}
    </label>
  );
}
