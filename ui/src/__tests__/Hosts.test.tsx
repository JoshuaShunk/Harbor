import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";
import { MemoryRouter } from "react-router-dom";
import Hosts from "../pages/Hosts";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

const mockedInvoke = vi.mocked(invoke);

function renderWithRouter(ui: React.ReactElement) {
  return render(<MemoryRouter>{ui}</MemoryRouter>);
}

describe("Hosts page", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("renders hosts returned from Tauri", async () => {
    mockedInvoke.mockResolvedValue({
      servers: [],
      hosts: [
        {
          name: "claude",
          display_name: "Claude Code",
          connected: true,
          config_exists: true,
          config_path: "~/.claude.json",
          server_count: 2,
        },
        {
          name: "vscode",
          display_name: "VS Code",
          connected: false,
          config_exists: true,
          config_path: ".vscode/mcp.json",
          server_count: 0,
        },
      ],
      gateway_port: 3100,
    });

    renderWithRouter(<Hosts />);

    await waitFor(() => {
      expect(screen.getByText("Claude Code")).toBeInTheDocument();
      expect(screen.getByText("VS Code")).toBeInTheDocument();
    });
  });

  it("shows Signal All button", async () => {
    mockedInvoke.mockResolvedValue({
      servers: [],
      hosts: [],
      gateway_port: 3100,
    });

    renderWithRouter(<Hosts />);

    await waitFor(() => {
      expect(screen.getByText("Signal All")).toBeInTheDocument();
    });
  });

  it("shows ship count for connected hosts", async () => {
    mockedInvoke.mockResolvedValue({
      servers: [],
      hosts: [
        {
          name: "claude",
          display_name: "Claude Code",
          connected: true,
          config_exists: true,
          config_path: "~/.claude.json",
          server_count: 3,
        },
      ],
      gateway_port: 3100,
    });

    renderWithRouter(<Hosts />);

    await waitFor(() => {
      expect(screen.getByText("3 ships signaled")).toBeInTheDocument();
    });
  });
});
