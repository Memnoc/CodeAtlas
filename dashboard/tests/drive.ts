// Driving the explorer the way a reader does. The affordances the tests
// exercise live behind the header's two switches and behind drilling into a
// region, so every suite needs the same three gestures; sharing them keeps
// each test about what it asserts rather than about how to get there.
import { screen, within } from "@testing-library/react";
import type userEvent from "@testing-library/user-event";

type User = ReturnType<typeof userEvent.setup>;

/** Switches the right panel to the guided tour. */
export async function openLearn(user: User): Promise<void> {
  await user.click(
    within(screen.getByRole("radiogroup", { name: "View" })).getByRole(
      "radio",
      { name: "Learn" },
    ),
  );
}

/** Switches the grouping to domains, which is where the flows panel lives. */
export async function openDomainGrouping(user: User): Promise<void> {
  await user.click(
    within(screen.getByRole("radiogroup", { name: "Grouping" })).getByRole(
      "radio",
      { name: "Domain" },
    ),
  );
}

/** Drills into a region by its display name, so the canvas draws its files. */
export async function openRegion(user: User, name: string): Promise<void> {
  const chips = screen
    .getAllByRole("button")
    .filter((b) => b.classList.contains("region-chip"));
  const chip = chips.find((b) => b.textContent?.startsWith(name));
  if (chip === undefined) {
    throw new Error(
      `no region chip named ${name}; saw ${chips.map((c) => c.textContent).join(", ")}`,
    );
  }
  await user.click(chip);
}

/** The node the canvas currently has selected, as React Flow marks it — and
 * never more than one, whoever made the selection. The canvas draws files,
 * so selecting a symbol marks the file that contains it. */
export function selectedOnCanvas(): string | null {
  const selected = document.querySelectorAll(".react-flow__node.selected");
  if (selected.length > 1) {
    throw new Error(`${selected.length} nodes selected at once`);
  }
  return selected[0]?.getAttribute("data-id") ?? null;
}
