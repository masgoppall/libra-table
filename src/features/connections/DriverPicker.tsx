// CON-3: choose the kind of database for a new connection, grouped like TablePlus.

import { useMemo, useState } from "react";
import { DriverLogo } from "@/components/DriverLogo";
import { Modal } from "@/components/Modal";
import { ModalActions } from "@/components/ModalActions";
import { DRIVER_GROUPS } from "@/domain/connection";
import { useWindowKeys } from "@/hooks/useWindowKeys";
import { useStore } from "@/store";
import { type Driver, type DriverEntry } from "@/types";

const NEXT_KEYS = ["ArrowRight", "ArrowDown"];
const PREV_KEYS = ["ArrowLeft", "ArrowUp"];

function matches(d: DriverEntry, query: string) {
  const q = query.trim().toLowerCase();
  return !q || d.label.toLowerCase().includes(q) || d.id.includes(q);
}

function step(list: DriverEntry[], current: Driver, dir: 1 | -1): Driver {
  const ready = list.filter((d) => d.ready);
  const i = ready.findIndex((d) => d.id === current);
  return ready[(i + dir + ready.length) % ready.length]?.id ?? current;
}

function DriverCard(props: {
  d: DriverEntry;
  selected: boolean;
  onSelect: () => void;
  onPick: () => void;
}) {
  const { d } = props;
  return (
    <button
      className={`driver-card${props.selected ? " selected" : ""}`}
      disabled={!d.ready}
      onClick={props.onSelect}
      onDoubleClick={props.onPick}
      title={d.ready ? d.label : `${d.label} is not available yet`}
    >
      <DriverLogo driver={d.id} size={36} />
      <div className="driver-name">{d.label}</div>
      <div className="driver-desc">{d.ready ? d.note : "Coming soon"}</div>
    </button>
  );
}

export function DriverPicker(props: { onPick: (d: Driver) => void; onClose: () => void }) {
  const drivers = useStore((s) => s.drivers);
  const [selected, setSelected] = useState<Driver>("postgres");
  const [query, setQuery] = useState("");
  const shown = useMemo(() => drivers.filter((d) => matches(d, query)), [drivers, query]);
  const pick = (id: Driver) => drivers.find((d) => d.id === id)?.ready && props.onPick(id);
  useWindowKeys(true, (e) => {
    if (NEXT_KEYS.includes(e.key)) setSelected(step(shown, selected, 1));
    else if (PREV_KEYS.includes(e.key)) setSelected(step(shown, selected, -1));
    else if (e.key === "Enter") pick(shown.some((d) => d.id === selected) ? selected : "postgres");
  });
  return (
    <Modal
      size="lg"
      title="New connection"
      subtitle="Choose the kind of database"
      onClose={props.onClose}
      footer={
        <ModalActions onCancel={props.onClose} onOk={() => pick(selected)} okLabel="Continue" />
      }
    >
      <input
        className="input driver-search"
        autoFocus
        placeholder="Search drivers"
        value={query}
        onChange={(e) => setQuery(e.target.value)}
      />
      {DRIVER_GROUPS.map((g) => {
        const items = shown.filter((d) => d.group === g.id);
        if (items.length === 0) return null;
        return (
          <section key={g.id} className="driver-group">
            <h3 className="driver-group-title">{g.label}</h3>
            <div className="driver-grid">
              {items.map((d) => (
                <DriverCard
                  key={d.id}
                  d={d}
                  selected={selected === d.id}
                  onSelect={() => setSelected(d.id)}
                  onPick={() => pick(d.id)}
                />
              ))}
            </div>
          </section>
        );
      })}
      {shown.length === 0 && <div className="message faint">No driver matches.</div>}
    </Modal>
  );
}
