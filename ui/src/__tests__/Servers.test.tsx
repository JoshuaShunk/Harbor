import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";
import { MemoryRouter } from "react-router-dom";
import Servers from "../pages/Servers";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

const mockedInvoke = vi.mocked(invoke);

function renderWithRouter(ui: React.ReactElement) {
  return render(<MemoryRouter>{ui}</MemoryRouter>);
}

describe("Servers page", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("shows empty state when no servers exist", async () => {
    mockedInvoke.mockResolvedValue({
      servers: [],
      hosts: [],
      gateway_port: 3100,
    });

    renderWithRouter(<Servers />);

    await waitFor(() => {
      expect(screen.getByText("The docks are empty")).toBeInTheDocument();
    });
  });

  it("renders server list returned from Tauri", async () => {
    mockedInvoke.mockResolvedValue({
      servers: [
        {
          name: "memory",
          enabled: true,
          running: false,
          pid: null,
          command: "npx @modelcontextprotocol/server-memory",
        },
      ],
      hosts: [],
      gateway_port: 3100,
    });

    renderWithRouter(<Servers />);

    await waitFor(() => {
      expect(screen.getByText("memory")).toBeInTheDocument();
    });
  });

  it("shows the Fleet heading", async () => {
    mockedInvoke.mockResolvedValue({
      servers: [],
      hosts: [],
      gateway_port: 3100,
    });

    renderWithRouter(<Servers />);

    await waitFor(() => {
      expect(screen.getByText("Fleet")).toBeInTheDocument();
    });
  });
});
