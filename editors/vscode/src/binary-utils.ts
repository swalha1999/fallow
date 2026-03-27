import * as fs from "node:fs";
import * as os from "node:os";
import * as path from "node:path";

export const getExecutableExtension = (): string =>
  os.platform() === "win32" ? ".exe" : "";

export const findBinaryInPath = (name: string): string | null => {
  const executableName = `${name}${getExecutableExtension()}`;
  const pathDirs = (process.env["PATH"] ?? "").split(path.delimiter);

  for (const dir of pathDirs) {
    const candidate = path.join(dir, executableName);
    if (fs.existsSync(candidate)) {
      return candidate;
    }
  }

  return null;
};
