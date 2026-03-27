export interface AnalysisCompleteParams {
  totalIssues: number;
  unusedFiles: number;
  unusedExports: number;
  unusedTypes: number;
  unusedDependencies: number;
  unusedDevDependencies: number;
  unusedOptionalDependencies: number;
  unusedEnumMembers: number;
  unusedClassMembers: number;
  unresolvedImports: number;
  unlistedDependencies: number;
  duplicateExports: number;
  typeOnlyDependencies: number;
  circularDependencies: number;
  duplicationPercentage: number;
  cloneGroups: number;
}

type SeverityKey =
  | "statusBarItem.errorBackground"
  | "statusBarItem.warningBackground";

interface BreakdownLine {
  readonly count: keyof AnalysisCompleteParams;
  readonly icon: string;
  readonly label: string;
}

const BREAKDOWN_LINES: ReadonlyArray<BreakdownLine> = [
  {
    count: "unresolvedImports",
    icon: "$(error)",
    label: "unresolved imports",
  },
  { count: "unusedFiles", icon: "$(warning)", label: "unused files" },
  { count: "unusedExports", icon: "$(warning)", label: "unused exports" },
  { count: "unusedTypes", icon: "$(info)", label: "unused types" },
  {
    count: "unusedDependencies",
    icon: "$(warning)",
    label: "unused dependencies",
  },
  {
    count: "unusedDevDependencies",
    icon: "$(warning)",
    label: "unused dev dependencies",
  },
  {
    count: "unusedOptionalDependencies",
    icon: "$(warning)",
    label: "unused optional dependencies",
  },
  {
    count: "unusedEnumMembers",
    icon: "$(info)",
    label: "unused enum members",
  },
  {
    count: "unusedClassMembers",
    icon: "$(info)",
    label: "unused class members",
  },
  {
    count: "unlistedDependencies",
    icon: "$(warning)",
    label: "unlisted dependencies",
  },
  {
    count: "duplicateExports",
    icon: "$(warning)",
    label: "duplicate exports",
  },
  {
    count: "typeOnlyDependencies",
    icon: "$(info)",
    label: "type-only dependencies",
  },
  {
    count: "circularDependencies",
    icon: "$(warning)",
    label: "circular dependencies",
  },
];

export const getDuplicationPercentage = (
  duplicationPercentage: number
): number => (Number.isFinite(duplicationPercentage) ? duplicationPercentage : 0);

export const buildStatusBarPartsFromLsp = (
  params: AnalysisCompleteParams
): string[] => [
  `${params.totalIssues} issues`,
  `${getDuplicationPercentage(params.duplicationPercentage).toFixed(1)}% duplication`,
];

export const getStatusBarSeverityKey = (
  params: AnalysisCompleteParams
): SeverityKey | null => {
  if (params.unresolvedImports > 0) {
    return "statusBarItem.errorBackground";
  }

  if (params.totalIssues > 0) {
    return "statusBarItem.warningBackground";
  }

  return null;
};

export const buildStatusBarTooltipMarkdown = (
  params: AnalysisCompleteParams
): string => {
  const lines: string[] = ["**Fallow** - Analysis Results\n"];
  const duplicationPercentage = getDuplicationPercentage(
    params.duplicationPercentage
  );

  for (const line of BREAKDOWN_LINES) {
    const count = params[line.count];
    if (typeof count === "number" && count > 0) {
      lines.push(`${line.icon} ${count} ${line.label}`);
    }
  }

  if (params.cloneGroups > 0) {
    lines.push(
      `$(copy) ${params.cloneGroups} clone groups (${duplicationPercentage.toFixed(1)}% duplication)`
    );
  }

  if (params.totalIssues === 0 && params.cloneGroups === 0) {
    lines.push("$(check) No issues found");
  }

  lines.push("\n---\n");
  lines.push(
    "[$(play) Run Analysis](command:fallow.analyze) · [$(wrench) Auto-Fix](command:fallow.fix) · [$(output) Output](command:fallow.showOutput)"
  );

  return lines.join("\n\n");
};
