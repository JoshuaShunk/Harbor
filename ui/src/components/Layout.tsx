import { useEffect } from "react";
import { NavLink, Outlet, useNavigate } from "react-router-dom";
import { Zap, Link2, Compass, Settings, ArrowDownCircle, Loader2 } from "lucide-react";
import logo from "../assets/logo.png";
import type { LucideIcon } from "lucide-react";
import { listen } from "@tauri-apps/api/event";
import { useUpdate } from "../contexts/UpdateContext";

interface NavItem {
  to: string;
  label: string;
  icon: LucideIcon;
}

const navItems: NavItem[] = [
  { to: "/servers", label: "Fleet", icon: Zap },
  { to: "/hosts", label: "Ports", icon: Link2 },
  { to: "/marketplace", label: "Scout", icon: Compass },
  { to: "/settings", label: "Helm", icon: Settings },
];

function Layout() {
  const { status, currentVersion, availableVersion, progress, downloadAndInstall } = useUpdate();
  const navigate = useNavigate();

  // Listen for native macOS menu events (settings navigation only — update check is handled in Rust)
  useEffect(() => {
    let cancelled = false;
    const setup = listen<string>("menu-navigate", (event) => {
      if (!cancelled) navigate(event.payload);
    });
    return () => {
      cancelled = true;
      setup.then((unlisten) => unlisten());
    };
  }, [navigate]);

  const showBanner = status === "available" || status === "downloading" || status === "ready";

  const downloadPercent =
    progress && progress.total > 0
      ? Math.round((progress.downloaded / progress.total) * 100)
      : null;

  return (
    <div className="flex h-screen bg-bg-app p-3 pr-0">
      {/* Sidebar */}
      <nav className="w-60 shrink-0 flex flex-col bg-bg-subtle rounded-2xl overflow-hidden">
        {/* Title bar / drag region */}
        <div
          data-tauri-drag-region
          className="h-12 flex items-center gap-2.5 px-5 border-b border-border-subtle"
        >
          <img src={logo} alt="Harbor" className="h-5 w-auto" />
          <span className="text-sm font-semibold tracking-tight text-text-primary">
            Harbor
          </span>
        </div>

        {/* Nav links */}
        <div className="flex-1 py-3 px-3 space-y-0.5">
          {navItems.map((item) => (
            <NavLink
              key={item.to}
              to={item.to}
              className={({ isActive }) =>
                `group flex items-center gap-3 px-3 py-2 rounded-md text-[13px] transition-colors duration-150 ${
                  isActive
                    ? "bg-bg-active text-text-primary font-medium"
                    : "text-text-secondary hover:text-text-primary hover:bg-bg-hover"
                }`
              }
            >
              <item.icon className="w-4 h-4 shrink-0" />
              {item.label}
            </NavLink>
          ))}
        </div>

        {/* Update banner */}
        {showBanner && (
          <div className="mx-3 mb-2 p-3 rounded-lg bg-accent-muted border border-accent/20 animate-fade-in">
            {status === "available" && (
              <>
                <div className="text-[11px] font-medium text-accent mb-1.5">
                  New voyage v{availableVersion} sighted
                </div>
                <button
                  onClick={downloadAndInstall}
                  className="w-full flex items-center justify-center gap-1.5 px-2 py-1.5 rounded-md text-[11px] font-medium bg-accent text-white hover:bg-accent-hover transition-colors duration-150"
                >
                  <ArrowDownCircle className="w-3 h-3" />
                  Update & Relaunch
                </button>
              </>
            )}
            {status === "downloading" && (
              <>
                <div className="text-[11px] font-medium text-accent mb-1.5 flex items-center gap-1.5">
                  <Loader2 className="w-3 h-3 animate-spin" />
                  Hauling cargo{downloadPercent !== null ? ` ${downloadPercent}%` : "..."}
                </div>
                {downloadPercent !== null && (
                  <div className="w-full h-1 rounded-full bg-bg-active overflow-hidden">
                    <div
                      className="h-full bg-accent rounded-full transition-all duration-300"
                      style={{ width: `${downloadPercent}%` }}
                    />
                  </div>
                )}
              </>
            )}
            {status === "ready" && (
              <div className="text-[11px] font-medium text-accent flex items-center gap-1.5">
                <Loader2 className="w-3 h-3 animate-spin" />
                Setting sail on new course...
              </div>
            )}
          </div>
        )}

        {/* Version footer */}
        <div className="px-5 py-3">
          <span className="text-[11px] text-text-muted">Harbor v{currentVersion}</span>
        </div>
      </nav>

      {/* Main content */}
      <main className="flex-1 overflow-auto bg-bg-app pl-3">
        <Outlet />
      </main>
    </div>
  );
}

export default Layout;
