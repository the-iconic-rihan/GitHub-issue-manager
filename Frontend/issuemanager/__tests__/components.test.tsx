import { act, render, renderHook, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import IssueFilters, { DEFAULT_FILTERS, filtersFromQuery, filtersToParams } from "@/components/IssueFilters";
import IssueForm from "@/components/IssueForm";
import Pagination from "@/components/Pagination";
import { LabelBadge, StateBadge } from "@/components/ui";
import { useApi } from "@/lib/useApi";

jest.mock("next-auth/react", () => ({ signOut: jest.fn() }));

describe("IssueForm", () => {
  const labels = [{ id: 1, name: "bug", color: "ff0000" }];
  const assignees = [{ id: 2, login: "octocat", avatar_url: "", html_url: "" }];

  it("blocks submission without a title and shows the error", async () => {
    const onSubmit = jest.fn();
    render(<IssueForm labels={labels} assignees={assignees} onSubmit={onSubmit} submitting={false} />);
    await userEvent.click(screen.getByRole("button", { name: "Create issue" }));
    expect(screen.getByText("Title is required.")).toBeInTheDocument();
    expect(screen.getByLabelText(/Title/)).toHaveAttribute("aria-invalid", "true");
    expect(onSubmit).not.toHaveBeenCalled();
  });

  it("submits trimmed values with selected labels and assignees", async () => {
    const onSubmit = jest.fn();
    render(<IssueForm labels={labels} assignees={assignees} onSubmit={onSubmit} submitting={false} />);
    await userEvent.type(screen.getByLabelText(/Title/), "  Crash on start  ");
    await userEvent.type(screen.getByLabelText("Description"), "Steps");
    await userEvent.click(screen.getByRole("checkbox", { name: "bug" }));
    await userEvent.click(screen.getByRole("checkbox", { name: "octocat" }));
    await userEvent.click(screen.getByRole("button", { name: "Create issue" }));
    expect(onSubmit).toHaveBeenCalledWith({ title: "Crash on start", body: "Steps", labels: ["bug"], assignees: ["octocat"] });
  });

  it("shows server-side field errors and the pending state", () => {
    render(<IssueForm labels={[]} assignees={[]} onSubmit={jest.fn()} submitting serverErrors={{ title: "is too long" }} />);
    expect(screen.getByText("is too long")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Creating…" })).toBeDisabled();
  });
});

describe("IssueFilters", () => {
  it("maps URL query to filters and filters to API params", () => {
    const f = filtersFromQuery({ state: "closed", label: "bug", assignee: "none", sort: "updated-asc", q: "crash" });
    expect(f).toEqual({ q: "crash", state: "closed", label: "bug", assignee: "none", sort: "updated-asc" });
    expect(filtersToParams(f)).toEqual({ q: "crash", state: "closed", labels: "bug", assignee: "none", sort: "updated", direction: "asc" });
    expect(filtersFromQuery({ state: "bogus", sort: "evil" })).toEqual(DEFAULT_FILTERS);
  });

  it("emits changes for state, label and search", async () => {
    const onChange = jest.fn();
    render(
      <IssueFilters
        value={DEFAULT_FILTERS}
        onChange={onChange}
        labels={[{ id: 1, name: "bug", color: "ff0000" }]}
        assignees={[]}
        myLogin="octocat"
      />,
    );
    await userEvent.click(screen.getByRole("button", { name: "closed" }));
    expect(onChange).toHaveBeenLastCalledWith({ ...DEFAULT_FILTERS, state: "closed" });
    await userEvent.selectOptions(screen.getByLabelText("Label"), "bug");
    expect(onChange).toHaveBeenLastCalledWith({ ...DEFAULT_FILTERS, label: "bug" });
    await userEvent.selectOptions(screen.getByLabelText("Assignee"), "Assigned to me");
    expect(onChange).toHaveBeenLastCalledWith({ ...DEFAULT_FILTERS, assignee: "octocat" });
    await userEvent.type(screen.getByRole("searchbox"), "crash{enter}");
    expect(onChange).toHaveBeenLastCalledWith({ ...DEFAULT_FILTERS, q: "crash" });
  });
});

describe("Pagination", () => {
  it("is hidden for a single page and disables unavailable directions", async () => {
    const onPage = jest.fn();
    const { rerender, container } = render(<Pagination meta={{ page: 1, per_page: 30, has_next: false }} onPage={onPage} />);
    expect(container).toBeEmptyDOMElement();

    rerender(<Pagination meta={{ page: 1, per_page: 30, has_next: true, total_count: 75 }} onPage={onPage} />);
    expect(screen.getByText("Page 1 of 3")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /Previous/ })).toBeDisabled();
    await userEvent.click(screen.getByRole("button", { name: /Next/ }));
    expect(onPage).toHaveBeenCalledWith(2);
  });
});

describe("badges", () => {
  it("renders state and label badges", () => {
    render(
      <>
        <StateBadge state="closed" reason="not_planned" />
        <LabelBadge name="bug" color="000000" />
      </>,
    );
    expect(screen.getByText("Closed (not planned)")).toBeInTheDocument();
    expect(screen.getByText("bug").parentElement).toHaveStyle({ color: "#ffffff" });
  });
});

describe("useApi", () => {
  const originalFetch = global.fetch;
  afterEach(() => {
    global.fetch = originalFetch;
  });

  function respondWith(byPath: Record<string, unknown>) {
    global.fetch = jest.fn(async (url: string) => ({
      status: 200,
      ok: true,
      headers: { get: () => null },
      json: async () => ({ data: byPath[url.replace("/api/backend/", "")] }),
    })) as unknown as typeof fetch;
  }

  it("never shows data from a different path while loading", async () => {
    respondWith({ "issues/1": { n: 1 }, "issues/2": { n: 2 } });
    const { result, rerender } = renderHook(({ path }) => useApi<{ n: number }>(path), { initialProps: { path: "issues/1" } });
    expect(result.current.loading).toBe(true);
    await waitFor(() => expect(result.current.data).toEqual({ n: 1 }));

    rerender({ path: "issues/2" });
    expect(result.current.loading).toBe(true);
    expect(result.current.data).toBeUndefined();
    await waitFor(() => expect(result.current.data).toEqual({ n: 2 }));
  });

  it("keeps previous data with keepPrevious and supports reload/setData/null paths", async () => {
    respondWith({ "list?page=1": [1], "list?page=2": [2] });
    const { result, rerender } = renderHook(({ path }) => useApi<number[]>(path, { keepPrevious: true }), {
      initialProps: { path: "list?page=1" as string | null },
    });
    await waitFor(() => expect(result.current.data).toEqual([1]));
    rerender({ path: "list?page=2" });
    expect(result.current.data).toEqual([1]);
    await waitFor(() => expect(result.current.data).toEqual([2]));

    act(() => result.current.setData((d) => [...(d ?? []), 3]));
    expect(result.current.data).toEqual([2, 3]);

    rerender({ path: null });
    expect(result.current.loading).toBe(false);
  });

  it("exposes errors", async () => {
    global.fetch = jest.fn(async () => ({
      status: 404,
      ok: false,
      headers: { get: () => null },
      json: async () => ({ error: { code: "not_found", message: "Missing" } }),
    })) as unknown as typeof fetch;
    const { result } = renderHook(() => useApi("x"));
    await waitFor(() => expect(result.current.error?.message).toBe("Missing"));
    expect(result.current.loading).toBe(false);
  });
});
