// What an empty workspace shows: a quiet illustration in the middle of the content area,
// as TablePlus does — a balance (Libra) weighing a database against a table.

export function WorkspaceWelcome() {
  return (
    <div className="ws-empty" aria-hidden>
      <svg className="ws-empty-art" width="267" height="184" viewBox="0 0 267 184">
        <g stroke="var(--ill-line)" strokeWidth="3.5" strokeLinecap="round" strokeLinejoin="round">
          <path d="M8 177.5h251" fill="none" />
          <path d="M104 177.5l9-17h41l9 17z" fill="var(--ill-fill)" />
          <rect x="128.5" y="40" width="10" height="121" rx="2" fill="var(--ill-fill-2)" />
          <circle cx="133.5" cy="33" r="8" fill="var(--ill-fill-2)" />
          <path d="M50 58.5l167-16.5" fill="none" strokeWidth="8" stroke="var(--ill-line)" />
          <path d="M50 58.5l167-16.5" fill="none" strokeWidth="2.5" stroke="var(--ill-fill-2)" />
          <path d="M50 58.5l-24 61M50 58.5l24 61M217 42l-24 59M217 42l24 59" fill="none" />
          <path d="M18 119.5h64c-3 13-15 18-32 18s-29-5-32-18z" fill="var(--ill-fill)" />
          <path d="M185 101h64c-3 13-15 18-32 18s-29-5-32-18z" fill="var(--ill-fill)" />
          <path d="M32 84v28c0 4 8 7.5 18 7.5s18-3.5 18-7.5V84" fill="var(--ill-fill-2)" />
          <path d="M32 98c0 4 8 7.5 18 7.5S68 102 68 98" fill="none" />
          <ellipse cx="50" cy="84" rx="18" ry="7" fill="var(--ill-fill)" />
          <rect x="194.5" y="67" width="45" height="34" rx="4" fill="var(--ill-fill-2)" />
          <path d="M194.5 77.5h45M209.5 77.5v23.5M224.5 77.5v23.5M194.5 89h45" fill="none" />
        </g>
        <rect x="211.5" y="79.5" width="11" height="7.5" rx="1.5" fill="var(--ill-accent)" />
        <circle cx="44" cy="83" r="2.2" fill="var(--ill-accent)" />
      </svg>
    </div>
  );
}
