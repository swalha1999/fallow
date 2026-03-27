// VS Code injects this module into the extension host at runtime.
// fallow-ignore-next-line unlisted-dependency
import * as vscode from "vscode";
import { countCheckIssues } from "./analysis-utils.js";
import {
  buildStatusBarPartsFromLsp,
  buildStatusBarTooltipMarkdown,
  getStatusBarSeverityKey,
} from "./statusBar-utils.js";
import type { FallowCheckResult, FallowDupesResult } from "./types.js";
export type { AnalysisCompleteParams } from "./statusBar-utils.js";
import type { AnalysisCompleteParams } from "./statusBar-utils.js";

let statusBarItem: vscode.StatusBarItem | null = null;

export const createStatusBar = (): vscode.StatusBarItem => {
  statusBarItem = vscode.window.createStatusBarItem(
    vscode.StatusBarAlignment.Left,
    50
  );
  statusBarItem.command = "fallow.analyze";
  statusBarItem.text = "$(search) Fallow";
  statusBarItem.show();
  return statusBarItem;
};

/** Update the status bar from CLI-driven analysis results. */
export const updateStatusBar = (
  checkResult: FallowCheckResult | null,
  dupesResult: FallowDupesResult | null
): void => {
  if (!statusBarItem) {
    return;
  }

  const parts: string[] = [];

  if (checkResult) {
    parts.push(`${countCheckIssues(checkResult)} issues`);
  }

  if (dupesResult) {
    const pct = dupesResult.stats.duplication_percentage.toFixed(1);
    parts.push(`${pct}% duplication`);
  }

  applyStatusBarText(parts);
};

/** Update the status bar from LSP notification data. */
export const updateStatusBarFromLsp = (params: AnalysisCompleteParams): void => {
  if (!statusBarItem) {
    return;
  }

  const severity = getStatusBarSeverityKey(params);
  statusBarItem.backgroundColor = severity
    ? new vscode.ThemeColor(severity)
    : undefined;

  const tooltip = new vscode.MarkdownString(
    buildStatusBarTooltipMarkdown(params)
  );
  tooltip.isTrusted = true;
  statusBarItem.tooltip = tooltip;

  applyStatusBarText(buildStatusBarPartsFromLsp(params));
};

const applyStatusBarText = (parts: string[]): void => {
  if (!statusBarItem) {
    return;
  }
  if (parts.length > 0) {
    statusBarItem.text = `$(search) Fallow: ${parts.join(" | ")}`;
  } else {
    statusBarItem.text = "$(search) Fallow";
  }
};

export const setStatusBarAnalyzing = (): void => {
  if (statusBarItem) {
    statusBarItem.text = "$(loading~spin) Fallow: Analyzing...";
  }
};

export const setStatusBarError = (): void => {
  if (statusBarItem) {
    statusBarItem.text = "$(error) Fallow: Error";
  }
};

export const disposeStatusBar = (): void => {
  if (statusBarItem) {
    statusBarItem.dispose();
    statusBarItem = null;
  }
};
