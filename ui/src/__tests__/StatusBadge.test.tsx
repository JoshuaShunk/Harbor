import { describe, it, expect } from "vitest";
import { render, screen } from "@testing-library/react";
import StatusBadge from "../components/StatusBadge";
import type { Status } from "../components/StatusBadge";

describe("StatusBadge", () => {
  const cases: { status: Status; label: string }[] = [
    { status: "running", label: "At Sea" },
    { status: "stopped", label: "Anchored" },
    { status: "enabled", label: "Rigged" },
    { status: "disabled", label: "Moored" },
    { status: "connected", label: "Linked" },
    { status: "detected", label: "Sighted" },
    { status: "not_found", label: "Uncharted" },
    { status: "chartered", label: "Chartered" },
    { status: "expired", label: "Papers Expired" },
    { status: "unchartered", label: "Unchartered" },
  ];

  it.each(cases)(
    "renders the correct label for $status",
    ({ status, label }) => {
      render(<StatusBadge status={status} />);
      expect(screen.getByText(label)).toBeInTheDocument();
    },
  );

  it("applies pulse animation only for running status", () => {
    const { container } = render(<StatusBadge status="running" />);
    const dot = container.querySelector(".animate-pulse-dot");
    expect(dot).toBeInTheDocument();
  });

  it("does not apply pulse animation for stopped status", () => {
    const { container } = render(<StatusBadge status="stopped" />);
    const dot = container.querySelector(".animate-pulse-dot");
    expect(dot).not.toBeInTheDocument();
  });
});
