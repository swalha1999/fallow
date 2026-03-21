import * as vscode from "vscode";
import { startClient, stopClient, restartClient } from "./client.js";
import { onConfigChange } from "./config.js";
import { runAnalysis, runFix } from "./commands.js";
import {
  createStatusBar,
  updateStatusBar,
  setStatusBarAnalyzing,
  setStatusBarError,
  disposeStatusBar,
} from "./statusBar.js";
import { DeadCodeTreeProvider, DuplicatesTreeProvider } from "./treeView.js";
import type { FallowCheckResult, FallowDupesResult } from "./types.js";

let outputChannel: vscode.OutputChannel;
let lastCheckResult: FallowCheckResult | null = null;
let lastDupesResult: FallowDupesResult | null = null;

export const activate = async (
  context: vscode.ExtensionContext
): Promise<void> => {
  outputChannel = vscode.window.createOutputChannel("Fallow");
  context.subscriptions.push(outputChannel);

  const statusBar = createStatusBar();
  context.subscriptions.push(statusBar);

  const deadCodeProvider = new DeadCodeTreeProvider();
  const duplicatesProvider = new DuplicatesTreeProvider();

  // Use createTreeView to get visibility events — defer CLI analysis until the
  // tree view is first shown, avoiding a double analysis on activation (the LSP
  // runs its own analysis for diagnostics).
  let cliAnalysisRan = false;

  const triggerCliAnalysis = async (): Promise<void> => {
    setStatusBarAnalyzing();
    try {
      const { check, dupes } = await runAnalysis(context);
      lastCheckResult = check;
      lastDupesResult = dupes;
      updateViews();
    } catch {
      setStatusBarError();
    }
  };

  const deadCodeView = vscode.window.createTreeView("fallow.deadCode", {
    treeDataProvider: deadCodeProvider,
  });
  const duplicatesView = vscode.window.createTreeView("fallow.duplicates", {
    treeDataProvider: duplicatesProvider,
  });
  context.subscriptions.push(deadCodeView, duplicatesView);

  const onViewVisible = (): void => {
    if (cliAnalysisRan) {
      return;
    }
    cliAnalysisRan = true;
    void triggerCliAnalysis();
  };

  context.subscriptions.push(
    deadCodeView.onDidChangeVisibility((e) => {
      if (e.visible) {
        onViewVisible();
      }
    })
  );
  context.subscriptions.push(
    duplicatesView.onDidChangeVisibility((e) => {
      if (e.visible) {
        onViewVisible();
      }
    })
  );

  const updateViews = (): void => {
    deadCodeProvider.update(lastCheckResult);
    duplicatesProvider.update(lastDupesResult);
    updateStatusBar(lastCheckResult, lastDupesResult);
  };

  // Register commands
  context.subscriptions.push(
    vscode.commands.registerCommand("fallow.analyze", async () => {
      cliAnalysisRan = true;
      await triggerCliAnalysis();
    })
  );

  context.subscriptions.push(
    vscode.commands.registerCommand("fallow.fix", async () => {
      await runFix(context, false);
      // Re-run analysis after fix
      cliAnalysisRan = true;
      await triggerCliAnalysis();
    })
  );

  context.subscriptions.push(
    vscode.commands.registerCommand("fallow.fixDryRun", async () => {
      await runFix(context, true);
    })
  );

  context.subscriptions.push(
    vscode.commands.registerCommand("fallow.restart", async () => {
      outputChannel.appendLine("Restarting language server...");
      await restartClient(context, outputChannel);
    })
  );

  context.subscriptions.push(
    vscode.commands.registerCommand("fallow.showOutput", () => {
      outputChannel.show();
    })
  );

  // Fallback command for Code Lens items with 0 references (display-only)
  context.subscriptions.push(
    vscode.commands.registerCommand("fallow.noop", () => {})
  );

  // Watch for config changes
  context.subscriptions.push(
    onConfigChange(async (e) => {
      const needsRestart =
        e.affectsConfiguration("fallow.lspPath") ||
        e.affectsConfiguration("fallow.trace.server") ||
        e.affectsConfiguration("fallow.issueTypes");

      const needsReanalysis =
        e.affectsConfiguration("fallow.production") ||
        e.affectsConfiguration("fallow.duplication") ||
        e.affectsConfiguration("fallow.issueTypes");

      if (needsRestart) {
        outputChannel.appendLine("Configuration changed, restarting server...");
        await restartClient(context, outputChannel);
      }

      if (needsReanalysis) {
        // Re-run CLI analysis for tree views and status bar
        // (sequenced after LSP restart if both apply)
        void triggerCliAnalysis();
      }
    })
  );

  // Start LSP client
  const client = await startClient(context, outputChannel);
  if (client) {
    context.subscriptions.push({ dispose: () => void stopClient() });
  }
};

export const deactivate = async (): Promise<void> => {
  disposeStatusBar();
  await stopClient();
};
