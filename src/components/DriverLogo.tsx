import { type Driver } from "@/types";

const DRIVER_STYLE: Record<Driver, { from: string; to: string; text: string }> = {
  postgres: { from: "#4b7bb5", to: "#2f5d8f", text: "Pg" },
  mysql: { from: "#1d8fb0", to: "#e48e00", text: "My" },
  sqlite: { from: "#3fa4dc", to: "#1a6aa6", text: "Lt" },
};

export function DriverLogo({ driver, size = 40 }: { driver: Driver; size?: number }) {
  const c = DRIVER_STYLE[driver];
  return (
    <div
      className="driver-logo"
      aria-hidden
      style={{
        background: `linear-gradient(140deg, ${c.from}, ${c.to})`,
        width: size,
        height: size,
        fontSize: Math.round(size * 0.4),
        borderRadius: Math.round(size * 0.28),
      }}
    >
      {c.text}
    </div>
  );
}
