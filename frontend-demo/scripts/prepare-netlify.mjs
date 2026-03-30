import { cpSync, existsSync, mkdirSync, rmSync, writeFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const projectRoot = path.resolve(scriptDir, "..");
const distDir = path.join(projectRoot, "dist");
const outputDir = path.join(projectRoot, "netlify-dist");
const siteDir = path.join(outputDir, "ossify");
const docsDir = path.resolve(projectRoot, "..", "docs");

if (!existsSync(distDir)) {
  throw new Error("Missing dist directory. Run `npm run build` before preparing Netlify output.");
}

rmSync(outputDir, { recursive: true, force: true });
mkdirSync(siteDir, { recursive: true });

cpSync(distDir, siteDir, { recursive: true });

for (const filename of ["install.ps1", "install.sh"]) {
  const sourcePath = path.join(docsDir, filename);
  cpSync(sourcePath, path.join(siteDir, filename));
  cpSync(sourcePath, path.join(outputDir, filename));
}

writeFileSync(
  path.join(outputDir, "index.html"),
  `<!doctype html>
<html lang="en">
  <head>
    <meta charset="utf-8" />
    <meta http-equiv="refresh" content="0; url=/ossify/" />
    <meta name="viewport" content="width=device-width, initial-scale=1" />
    <title>ossify</title>
  </head>
  <body>
    <p>Redirecting to <a href="/ossify/">/ossify/</a>...</p>
  </body>
</html>
`,
);
