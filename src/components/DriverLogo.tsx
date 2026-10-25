import { type Driver } from "@/types";

/** Monogram badges: two brand-like colors and a short mark per driver. */
const DRIVER_STYLE: Record<Driver, { from: string; to: string; text: string }> = {
  postgres: { from: "#4b7bb5", to: "#2f5d8f", text: "Pg" },
  mysql: { from: "#1d8fb0", to: "#e48e00", text: "My" },
  sqlite: { from: "#3fa4dc", to: "#1a6aa6", text: "Lt" },
  mariadb: { from: "#1f3b57", to: "#c49a6c", text: "Ma" },
  cockroach: { from: "#6933ff", to: "#3b1d8f", text: "Cr" },
  redshift: { from: "#8c4fff", to: "#2e27ad", text: "Rs" },
  greenplum: { from: "#3ab78d", to: "#1d6b52", text: "Gp" },
  vertica: { from: "#2f6fdf", to: "#0b2f73", text: "Vt" },
  libsql: { from: "#4ff8d2", to: "#1a8f77", text: "Ls" },
  cloudflare_d1: { from: "#f6821f", to: "#c4531a", text: "D1" },
  mssql: { from: "#d83b01", to: "#a4262c", text: "Ms" },
  oracle: { from: "#f80000", to: "#9e0b0b", text: "Or" },
  clickhouse: { from: "#faff69", to: "#c9a800", text: "Ch" },
  duckdb: { from: "#fff100", to: "#d4b800", text: "Dk" },
  cassandra: { from: "#1287b1", to: "#0b4f6c", text: "Ca" },
  snowflake: { from: "#29b5e8", to: "#1a7bb0", text: "Sf" },
  bigquery: { from: "#4386fa", to: "#2a5bb8", text: "Bq" },
  redis: { from: "#dc382d", to: "#a41e11", text: "Rd" },
  mongodb: { from: "#47a248", to: "#2a6b2e", text: "Mg" },
  dynamodb: { from: "#4053d6", to: "#2a2f8f", text: "Dy" },
  elasticsearch: { from: "#fec514", to: "#00a9a5", text: "Es" },
};

/** Light backgrounds need dark text. */
const DARK_TEXT: Driver[] = ["clickhouse", "duckdb", "libsql", "elasticsearch"];

export function DriverLogo({ driver, size = 40 }: { driver: Driver; size?: number }) {
  const c = DRIVER_STYLE[driver];
  return (
    <div
      className="driver-logo"
      aria-hidden
      style={{
        background: `linear-gradient(140deg, ${c.from}, ${c.to})`,
        color: DARK_TEXT.includes(driver) ? "#1b1b1b" : undefined,
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
