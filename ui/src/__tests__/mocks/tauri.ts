import { vi } from "vitest";

// Mock @tauri-apps/api/core so components that call invoke() work in tests
vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));
