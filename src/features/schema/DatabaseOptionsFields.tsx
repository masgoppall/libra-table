// Server options of a new database: encoding/template/owner (Postgres) or
// character set/collation (MySQL).

const PG_ENCODINGS = ["", "UTF8", "LATIN1", "SQL_ASCII", "WIN1252"];
const PG_TEMPLATES = ["", "template0", "template1"];
const MYSQL_CHARSETS = ["", "utf8mb4", "utf8mb3", "latin1", "ascii", "binary"];
const MYSQL_COLLATIONS = [
  "utf8mb4_0900_ai_ci",
  "utf8mb4_unicode_ci",
  "utf8mb4_general_ci",
  "utf8mb4_bin",
  "latin1_swedish_ci",
];

export interface OptionsDraft {
  charset: string;
  collation: string;
  encoding: string;
  template: string;
  owner: string;
}

function Choice(props: {
  label: string;
  value: string;
  choices: string[];
  emptyLabel: string;
  onChange: (v: string) => void;
}) {
  return (
    <>
      <label>{props.label}</label>
      <select
        className="select"
        value={props.value}
        onChange={(e) => props.onChange(e.target.value)}
      >
        {props.choices.map((v) => (
          <option key={v} value={v}>
            {v || props.emptyLabel}
          </option>
        ))}
      </select>
    </>
  );
}

export function DatabaseOptionsFields(props: {
  postgres: boolean;
  value: OptionsDraft;
  onChange: (o: OptionsDraft) => void;
}) {
  const o = props.value;
  const set = (patch: Partial<OptionsDraft>) => props.onChange({ ...o, ...patch });
  if (props.postgres) {
    return (
      <>
        <Choice
          label="Encoding"
          value={o.encoding}
          choices={PG_ENCODINGS}
          emptyLabel="Server default"
          onChange={(encoding) => set({ encoding })}
        />
        <Choice
          label="Template"
          value={o.template}
          choices={PG_TEMPLATES}
          emptyLabel="Default (template1)"
          onChange={(template) => set({ template })}
        />
        <label>Owner</label>
        <input
          className="input"
          value={o.owner}
          placeholder="current user"
          onChange={(e) => set({ owner: e.target.value })}
        />
      </>
    );
  }
  const collations = MYSQL_COLLATIONS.filter((c) => !o.charset || c.startsWith(o.charset));
  return (
    <>
      <Choice
        label="Character set"
        value={o.charset}
        choices={MYSQL_CHARSETS}
        emptyLabel="Server default"
        onChange={(charset) => set({ charset })}
      />
      <label>Collation</label>
      <input
        className="input"
        list="lt-collations"
        value={o.collation}
        placeholder="default for the character set"
        onChange={(e) => set({ collation: e.target.value })}
      />
      <datalist id="lt-collations">
        {collations.map((c) => (
          <option key={c} value={c} />
        ))}
      </datalist>
    </>
  );
}
