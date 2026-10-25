import {
  Columns3,
  Copy,
  Eye,
  Link2,
  ListTree,
  type LucideIcon,
  RefreshCw,
  Table2,
} from "lucide-react";
import { BusyIcon } from "@/components/Spinner";
import { withShortcut } from "@/lib/actions";
import { live } from "./designerModel";
import { type Section, type TableDesignerState } from "./useTableDesigner";

interface SectionTab {
  key: Section;
  label: string;
  icon: LucideIcon;
  count: number | null;
}

function sectionTabs(d: TableDesignerState): SectionTab[] {
  const tabs: SectionTab[] = [
    { key: "columns", label: "Columns", icon: Columns3, count: live(d.cols).length },
    { key: "indexes", label: "Indexes", icon: ListTree, count: live(d.idx).length },
    { key: "foreign", label: "Foreign keys", icon: Link2, count: live(d.fks).length },
  ];
  if (!d.creating) tabs.push({ key: "ddl", label: "DDL", icon: Copy, count: null });
  return tabs;
}

function Title({ d, where }: { d: TableDesignerState; where: string }) {
  return (
    <span className="toolbar-title">
      {d.isView ? <Eye size={14} /> : <Table2 size={14} />}
      {d.creating ? (
        <input
          className="input sm title-input"
          autoFocus
          placeholder="table_name"
          value={d.name}
          onChange={(e) => d.setName(e.target.value)}
        />
      ) : (
        <strong>{d.name}</strong>
      )}
      <span className="faint">{where}</span>
    </span>
  );
}

export function DesignerToolbar(props: {
  d: TableDesignerState;
  where: string;
  onOpenData: () => void;
}) {
  const { d } = props;
  return (
    <div className="toolbar">
      <Title d={d} where={props.where} />
      {!d.isView && (
        <div className="segmented">
          {sectionTabs(d).map(({ key, label, icon: Icon, count }) => (
            <button
              key={key}
              className={d.section === key ? "active" : ""}
              onClick={() => d.setSection(key)}
            >
              <Icon size={13} /> {label}
              {count !== null && <span className="count">{count}</span>}
            </button>
          ))}
        </div>
      )}
      <span className="spacer" />
      {!d.creating && (
        <>
          <button
            className="btn sm"
            title={withShortcut("Open the data", "open-structure")}
            onClick={props.onOpenData}
          >
            <Table2 size={13} /> Data
          </button>
          <button
            className="btn sm icon"
            title={withShortcut("Reload", "refresh")}
            onClick={() => void d.load()}
          >
            <BusyIcon busy={d.loading} icon={<RefreshCw size={13} />} />
          </button>
        </>
      )}
    </div>
  );
}
