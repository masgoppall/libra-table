// Which copy of pg_dump / mysqldump / … runs: the one found for the server's version,
// another one found, or a program the user picks (remembered for next time).

import { open } from "@tauri-apps/plugin-dialog";
import { AlertTriangle, FolderOpen } from "lucide-react";
import { api } from "@/api";
import { showError } from "@/services/errors";
import { type ToolChoice } from "@/types";

const AUTO = "";

export function ToolField(props: { tool: ToolChoice; onChanged: () => void; disabled?: boolean }) {
  const { tool } = props;
  const setPath = async (path: string | null) => {
    try {
      await api.setToolPath(tool.tool, path);
      props.onChanged();
    } catch (e) {
      showError(e);
    }
  };
  const choose = async () => {
    const picked = await open({ multiple: false, directory: false, title: `Choose ${tool.tool}` });
    if (typeof picked === "string") await setPath(picked);
  };
  const listed = tool.candidates.some((c) => c.path === tool.path);
  return (
    <>
      <label>{tool.tool}</label>
      <div className="col tight">
        <div className="row tight">
          <select
            className="select grow mono"
            aria-label={`${tool.tool} program`}
            value={tool.custom ? (tool.path ?? AUTO) : AUTO}
            disabled={props.disabled}
            onChange={(e) => void setPath(e.target.value || null)}
          >
            <option value={AUTO}>
              Automatic{!tool.custom && tool.path ? `: ${tool.path}` : ""}
            </option>
            {tool.candidates.map((c) => (
              <option key={c.path} value={c.path}>
                {c.path} ({c.major})
              </option>
            ))}
            {tool.custom && tool.path && !listed && <option value={tool.path}>{tool.path}</option>}
          </select>
          <button className="btn" onClick={() => void choose()} disabled={props.disabled}>
            <FolderOpen size={13} /> Choose…
          </button>
        </div>
        {tool.version && <span className="faint small">{tool.version}</span>}
        {tool.warning && (
          <span className="warn-text small row tight">
            <AlertTriangle size={12} /> {tool.warning}
          </span>
        )}
      </div>
    </>
  );
}
