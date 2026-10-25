import { Loader2 } from "lucide-react";

export function Spinner({ size = 14, className = "" }: { size?: number; className?: string }) {
  return <Loader2 size={size} className={`spin ${className}`.trim()} />;
}

/** Spinner while `busy`, else the icon. */
export function BusyIcon(props: { busy: boolean; icon: React.ReactNode; size?: number }) {
  return props.busy ? <Spinner size={props.size ?? 13} /> : <>{props.icon}</>;
}
