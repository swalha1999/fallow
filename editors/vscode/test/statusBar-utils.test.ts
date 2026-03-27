import { describe, expect, it } from "vitest";
import {
  buildStatusBarPartsFromLsp,
  buildStatusBarTooltipMarkdown,
  getStatusBarSeverityKey,
  getDuplicationPercentage,
} from "../src/statusBar-utils.js";
import type { AnalysisCompleteParams } from "../src/statusBar-utils.js";

const baseParams = (
  overrides: Partial<AnalysisCompleteParams> = {}
): AnalysisCompleteParams => ({
  totalIssues: 0,
  unusedFiles: 0,
  unusedExports: 0,
  unusedTypes: 0,
  unusedDependencies: 0,
  unusedDevDependencies: 0,
  unusedOptionalDependencies: 0,
  unusedEnumMembers: 0,
  unusedClassMembers: 0,
  unresolvedImports: 0,
  unlistedDependencies: 0,
  duplicateExports: 0,
  typeOnlyDependencies: 0,
  circularDependencies: 0,
  duplicationPercentage: 0,
  cloneGroups: 0,
  ...overrides,
});

describe("getDuplicationPercentage", () => {
  it("clamps non-finite values to zero", () => {
    expect(getDuplicationPercentage(Number.NaN)).toBe(0);
    expect(getDuplicationPercentage(Number.POSITIVE_INFINITY)).toBe(0);
  });

  it("keeps finite values unchanged", () => {
    expect(getDuplicationPercentage(4.25)).toBe(4.25);
  });
});

describe("buildStatusBarPartsFromLsp", () => {
  it("builds issue and duplication summary parts", () => {
    expect(
      buildStatusBarPartsFromLsp(
        baseParams({ totalIssues: 3, duplicationPercentage: 1.234 })
      )
    ).toEqual(["3 issues", "1.2% duplication"]);
  });
});

describe("getStatusBarSeverityKey", () => {
  it("prefers error styling for unresolved imports", () => {
    expect(
      getStatusBarSeverityKey(
        baseParams({ totalIssues: 2, unresolvedImports: 1 })
      )
    ).toBe("statusBarItem.errorBackground");
  });

  it("uses warning styling when issues exist without unresolved imports", () => {
    expect(
      getStatusBarSeverityKey(baseParams({ totalIssues: 2 }))
    ).toBe("statusBarItem.warningBackground");
  });

  it("returns null when there are no issues", () => {
    expect(getStatusBarSeverityKey(baseParams())).toBeNull();
  });
});

describe("buildStatusBarTooltipMarkdown", () => {
  it("includes only present issue categories and action links", () => {
    const markdown = buildStatusBarTooltipMarkdown(
      baseParams({
        totalIssues: 4,
        unusedFiles: 1,
        unresolvedImports: 2,
        cloneGroups: 1,
        duplicationPercentage: 3.25,
      })
    );

    expect(markdown).toContain("**Fallow** - Analysis Results");
    expect(markdown).toContain("$(error) 2 unresolved imports");
    expect(markdown).toContain("$(warning) 1 unused files");
    expect(markdown).toContain("$(copy) 1 clone groups (3.3% duplication)");
    expect(markdown).toContain("command:fallow.analyze");
    expect(markdown).not.toContain("unused exports");
  });

  it("shows a success message when no issues or clones exist", () => {
    const markdown = buildStatusBarTooltipMarkdown(baseParams());

    expect(markdown).toContain("$(check) No issues found");
  });
});
