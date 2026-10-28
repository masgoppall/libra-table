// New / edit group, as TablePlus asks for it: the icon on the left (click or drop an image
// to use your own, "Clear icon" for the plug again) and the name on the right.

import { open } from "@tauri-apps/plugin-dialog";
import { useState } from "react";
import { api } from "@/api";
import { Sheet } from "@/components/Sheet";
import { showError } from "@/services/errors";
import { type GroupEditor, store } from "@/store";
import { saveGroup } from "../connectionActions";
import { PlugCircleIcon } from "./icons";

/** Icons are kept at 48 pt for 2× screens. */
const ICON_PX = 96;
const IMAGE_EXTENSIONS = ["png", "jpg", "jpeg", "gif", "webp", "svg", "bmp", "tif", "tiff"];

/** Scales an image into a square PNG (letterboxed, transparent), small enough to save. */
function toIcon(src: string): Promise<string> {
  return new Promise((resolve, reject) => {
    const img = new Image();
    img.onload = () => {
      const canvas = document.createElement("canvas");
      canvas.width = canvas.height = ICON_PX;
      const ctx = canvas.getContext("2d");
      if (!ctx) return reject(new Error("Can't draw the image."));
      const scale = Math.min(ICON_PX / img.naturalWidth, ICON_PX / img.naturalHeight) || 1;
      const w = img.naturalWidth * scale;
      const h = img.naturalHeight * scale;
      ctx.imageSmoothingQuality = "high";
      ctx.drawImage(img, (ICON_PX - w) / 2, (ICON_PX - h) / 2, w, h);
      resolve(canvas.toDataURL("image/png"));
    };
    img.onerror = () => reject(new Error("This file isn't an image LibraTable can read."));
    img.src = src;
  });
}

function readFile(file: File): Promise<string> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => resolve(String(reader.result));
    reader.onerror = () => reject(reader.error);
    reader.readAsDataURL(file);
  });
}

export function GroupDialog({ editor }: { editor: GroupEditor }) {
  const g = editor.group;
  const [name, setName] = useState(g?.name ?? "");
  const [icon, setIcon] = useState<string | null>(g?.icon ?? null);
  const [over, setOver] = useState(false);
  const [busy, setBusy] = useState(false);
  const close = () => store.get().setGroupEditor(null);

  const applyImage = async (load: () => Promise<string>) => {
    try {
      setIcon(await toIcon(await load()));
    } catch (e) {
      showError(e);
    }
  };
  const choose = async () => {
    const path = await open({
      multiple: false,
      directory: false,
      title: "Choose an icon",
      filters: [{ name: "Images", extensions: IMAGE_EXTENSIONS }],
    });
    if (typeof path === "string") await applyImage(() => api.readImageFile(path));
  };
  const onDrop = (e: React.DragEvent) => {
    e.preventDefault();
    setOver(false);
    const file = e.dataTransfer.files[0];
    if (file) void applyImage(() => readFile(file));
  };
  const save = async () => {
    if (!name.trim() || busy) return;
    setBusy(true);
    const group = {
      id: g?.id ?? "",
      name: name.trim(),
      parentId: editor.parentId,
      collapsed: g?.collapsed ?? false,
      icon,
    };
    if (await saveGroup(group)) close();
    else setBusy(false);
  };

  return (
    <Sheet
      label={g ? "Edit group" : "New group"}
      className="group-sheet w-tokens"
      onClose={close}
      footer={
        <>
          <span className="spacer" />
          <button className="sheet-btn" onClick={close}>
            Cancel
          </button>
          <button
            className="sheet-btn primary"
            disabled={!name.trim() || busy}
            onClick={() => void save()}
          >
            Save
          </button>
        </>
      }
    >
      <div className="group-body">
        <div className="group-icon-col">
          <button
            className={`group-icon-well${over ? " over" : ""}`}
            title="Click or drop an image to use it as the icon"
            aria-label="Choose an icon"
            onClick={() => void choose()}
            onDragOver={(e) => {
              e.preventDefault();
              setOver(true);
            }}
            onDragLeave={() => setOver(false)}
            onDrop={onDrop}
          >
            {icon ? <img src={icon} alt="" draggable={false} /> : <PlugCircleIcon size={48} />}
          </button>
          <button className="group-clear" disabled={!icon} onClick={() => setIcon(null)}>
            Clear icon
          </button>
        </div>
        <label className="group-field">
          <span className="group-label">Group name</span>
          <input
            className="group-input"
            autoFocus
            value={name}
            placeholder="Production servers..."
            onChange={(e) => setName(e.target.value)}
            onKeyDown={(e) => e.key === "Enter" && void save()}
          />
        </label>
      </div>
    </Sheet>
  );
}
