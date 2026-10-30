// Custom tags of a connection: colored chips (welcome list) and the picker (form).

import { useStore } from "@/store";
import { type CustomTag } from "@/types";

const NO_TAGS: CustomTag[] = [];

export function useCustomTags(): CustomTag[] {
  return useStore((s) => s.settings?.customTags ?? NO_TAGS);
}

/** Chips for the labels that still exist in Settings. */
export function LabelChips({ labels }: { labels?: string[] }) {
  const tags = useCustomTags();
  const shown = tags.filter((t) => labels?.includes(t.name));
  return (
    <>
      {shown.map((t) => (
        <span
          key={t.name}
          className="label-chip"
          style={{ "--chip": t.color } as React.CSSProperties}
        >
          {t.name}
        </span>
      ))}
    </>
  );
}

export function LabelPicker(props: { labels: string[]; onChange: (labels: string[]) => void }) {
  const tags = useCustomTags();
  if (!tags.length) {
    return <span className="faint small">Add tags in Settings › Tags.</span>;
  }
  const toggle = (name: string) =>
    props.onChange(
      props.labels.includes(name)
        ? props.labels.filter((l) => l !== name)
        : [...props.labels, name],
    );
  return (
    <div className="row wrap">
      {tags.map((t) => {
        const on = props.labels.includes(t.name);
        return (
          <button
            key={t.name}
            type="button"
            className={`label-chip pick${on ? " on" : ""}`}
            aria-pressed={on}
            style={{ "--chip": t.color } as React.CSSProperties}
            onClick={() => toggle(t.name)}
          >
            {t.name}
          </button>
        );
      })}
    </div>
  );
}
