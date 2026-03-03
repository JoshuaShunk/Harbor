type Status = "running" | "stopped" | "enabled" | "disabled" | "connected" | "detected" | "not_found" | "chartered" | "expired" | "unchartered";

const variants: Record<Status, string> = {
  running: "bg-green-muted text-green",
  stopped: "bg-bg-active text-text-muted",
  enabled: "bg-green-muted text-green",
  disabled: "bg-bg-active text-text-muted",
  connected: "bg-green-muted text-green",
  detected: "bg-yellow-muted text-yellow",
  not_found: "bg-bg-active text-text-muted",
  chartered: "bg-green-muted text-green",
  expired: "bg-yellow-muted text-yellow",
  unchartered: "bg-bg-active text-text-muted",
};

const statusLabels: Record<Status, string> = {
  running: "At Sea",
  stopped: "Anchored",
  enabled: "Rigged",
  disabled: "Moored",
  connected: "Linked",
  detected: "Sighted",
  not_found: "Uncharted",
  chartered: "Chartered",
  expired: "Papers Expired",
  unchartered: "Unchartered",
};

function StatusBadge({ status }: { status: Status }) {
  const label = statusLabels[status] ?? status;
  const classes = variants[status] ?? "bg-bg-active text-text-muted";

  return (
    <span className={`inline-flex items-center gap-1.5 text-[11px] font-medium px-2 py-0.5 rounded-full ${classes}`}>
      <span
        className={`w-1.5 h-1.5 rounded-full bg-current ${
          status === "running" ? "animate-pulse-dot" : ""
        }`}
      />
      {label}
    </span>
  );
}

export default StatusBadge;
export type { Status };
