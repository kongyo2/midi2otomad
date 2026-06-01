import { afterEach, describe, expect, it, vi } from "vitest";

const createRoot = vi.hoisted(() => vi.fn(() => ({ render: vi.fn() })));

vi.mock("react-dom/client", () => ({ createRoot }));
vi.mock("./App", () => ({ App: () => null }));
vi.mock("./state/StudioContext", () => ({ StudioProvider: ({ children }: { children: React.ReactNode }) => children }));
vi.mock("./index.css", () => ({}));

afterEach(() => {
  vi.resetModules();
  vi.clearAllMocks();
  document.body.innerHTML = "";
});

describe("main entry", () => {
  it("mounts the app into #root", async () => {
    const root = document.createElement("div");
    root.id = "root";
    document.body.appendChild(root);

    await import("./main");

    expect(createRoot).toHaveBeenCalledWith(root);
    const instance = createRoot.mock.results[0]!.value as { render: ReturnType<typeof vi.fn> };
    expect(instance.render).toHaveBeenCalledTimes(1);
  });

  it("throws when #root is missing", async () => {
    await expect(import("./main")).rejects.toThrow("#root element not found");
  });
});
