// CON-3: choose the kind of database for a new connection.

import { useState } from "react";
import { DriverLogo } from "@/components/DriverLogo";
import { Modal } from "@/components/Modal";
import { ModalActions } from "@/components/ModalActions";
import { DRIVERS } from "@/domain/connection";
import { useWindowKeys } from "@/hooks/useWindowKeys";
import { type Driver } from "@/types";

const NEXT_KEYS = ["ArrowRight", "ArrowDown"];
const PREV_KEYS = ["ArrowLeft", "ArrowUp"];

function step(current: Driver, dir: 1 | -1): Driver {
  const i = DRIVERS.findIndex((d) => d.id === current);
  return DRIVERS[(i + dir + DRIVERS.length) % DRIVERS.length]?.id ?? current;
}

export function DriverPicker(props: { onPick: (d: Driver) => void; onClose: () => void }) {
  const [selected, setSelected] = useState<Driver>("postgres");
  useWindowKeys(true, (e) => {
    if (NEXT_KEYS.includes(e.key)) setSelected(step(selected, 1));
    else if (PREV_KEYS.includes(e.key)) setSelected(step(selected, -1));
    else if (e.key === "Enter") props.onPick(selected);
  });
  return (
    <Modal
      title="New connection"
      subtitle="Choose the kind of database"
      onClose={props.onClose}
      footer={
        <ModalActions
          leading={<span className="faint small">More drivers arrive after the MVP.</span>}
          onCancel={props.onClose}
          onOk={() => props.onPick(selected)}
          okLabel="Continue"
        />
      }
    >
      <div className="driver-grid">
        {DRIVERS.map((d) => (
          <button
            key={d.id}
            className={`driver-card${selected === d.id ? " selected" : ""}`}
            onClick={() => setSelected(d.id)}
            onDoubleClick={() => props.onPick(d.id)}
          >
            <DriverLogo driver={d.id} size={44} />
            <div className="driver-name">{d.label}</div>
            <div className="driver-desc">{d.description}</div>
          </button>
        ))}
      </div>
    </Modal>
  );
}
