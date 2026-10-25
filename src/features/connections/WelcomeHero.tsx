import { Link2, Plus } from "lucide-react";
import { Kbd } from "@/components/Kbd";
import { shortcutFor } from "@/lib/actions";

/** The first screen, before any connection exists. */
export function WelcomeHero(props: { onNewConnection: () => void; onImportUrl: () => void }) {
  return (
    <div className="hero">
      <div className="hero-mark">
        <img
          src="/app-icon.png"
          alt=""
          width={72}
          height={72}
          onError={(e) => (e.currentTarget.style.display = "none")}
        />
      </div>
      <h2>Welcome to LibraTable</h2>
      <p className="muted">
        Connect to PostgreSQL, MySQL or SQLite — directly or through an SSH tunnel. Passwords stay
        in your Keychain.
      </p>
      <div className="row">
        <button className="btn primary lg" onClick={props.onNewConnection}>
          <Plus size={15} /> New connection
        </button>
        <button className="btn lg" onClick={props.onImportUrl}>
          <Link2 size={15} /> Import from URL
        </button>
      </div>
      <div className="hero-tips faint">
        <span>
          <Kbd>{shortcutFor("new-connection")}</Kbd> new connection
        </span>
        <span>
          <Kbd>{shortcutFor("shortcuts")}</Kbd> all shortcuts
        </span>
      </div>
    </div>
  );
}
